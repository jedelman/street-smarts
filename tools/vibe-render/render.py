#!/usr/bin/env python3
"""Extrude a street-smarts Neighborhood JSON into real 3D solids (via
cadquery/OpenCascade -- the same B-rep kernel FreeCAD is built on; FreeCAD
itself isn't installable in this environment) and render floor-plan and
isometric views, plus a `.glb` for interactive viewing. Still a gut check
on scale and density, not a finished architectural rendering -- but window
and door
openings ARE now real OpenCascade boolean cuts (`punch_openings`), driven
by `p221_natural_doors_and_windows`'s pattern-derived placement, not
decoration. What's still NOT here: real wall thickness on the EXTERIOR
walls (a punch just pierces solid mass -- see `punch_openings`'s own
caveat) and roof forms.

Interior rooms are a separate, 2D concern: `render_floor_plan` draws each
building's `interior_cells` polygons directly (`p127_intimacy_gradient` /
`p129_common_areas_at_the_heart` / `p131_the_flow_through_rooms`'s cell
graph) as plain line art, with a door-width gap wherever two cells
connect -- see that function's own module doc for why this isn't built
into the 3D solid (a first attempt unioning wall slabs into the extruded
mass turned out to be geometrically inert: the mass has no room voids to
divide). Ground floor only, since no staircase pattern exists yet to reach
an upper one.

Also exports a single `.glb` (binary glTF) per scenario, colored the same
as the isometric render, with every building's real punched openings --
drop it into any standard glTF viewer (web three.js/`<model-viewer>`,
Blender, VS Code's 3D preview, an online glTF viewer) directly, without
re-running this pipeline just to look at the model again.

Input is the JSON a pattern pipeline run produces -- see
`crates/street-smarts-patterns/examples/dump_pipeline.rs`, or
`scripts/vibe-render.sh` for the end-to-end orchestration (Rust dump ->
this script) across both baseline scenarios.
"""
import json
import math
import sys
import warnings

import cadquery as cq
import matplotlib.colors as mcolors
import matplotlib.pyplot as plt
from matplotlib.patches import Patch
from mpl_toolkits.mplot3d.art3d import Poly3DCollection
import numpy as np

M_PER_DEG_LAT = 110_540.0
M_PER_DEG_LNG = 111_320.0

DEFAULT_BUILDING_HEIGHT_M = 9.0  # ~3 stories, for pads P107 didn't shape
STREET_THICKNESS_M = 0.3
PLAZA_THICKNESS_M = 0.15
FLOOR_TO_FLOOR_M = 3.5  # must match p221_natural_doors_and_windows's own default
OPENING_PUNCH_DEPTH_M = 3.0  # generous -- pierces any real wall thickness this massing model doesn't have
INTERIOR_DOOR_WIDTH_M = 0.9  # floor-plan door-gap width, drawn in-plane -- no wall thickness/height in a 2D plan
INTERIOR_WALL_MIN_LENGTH_M = 1.2  # shorter than this + a door gap leaves no real wall -- skip it
DEFAULT_CONTEXT_HEIGHT_M = 6.0  # ~2 stories, for context buildings Overture has no height for
CONTEXT_MIN_AREA_M2 = 8.0  # drop slivers/artifacts smaller than a garden shed


def project(lng, lat, origin_lng, origin_lat):
    x = (lng - origin_lng) * M_PER_DEG_LNG * math.cos(math.radians(origin_lat))
    y = (lat - origin_lat) * M_PER_DEG_LAT
    return x, y


def ring_to_xy(ring, origin_lng, origin_lat):
    pts = [project(p["lng"], p["lat"], origin_lng, origin_lat) for p in ring]
    # Drop duplicate closing point if present.
    if len(pts) >= 2 and pts[0] == pts[-1]:
        pts = pts[:-1]
    return pts


def load(path):
    with open(path) as f:
        return json.load(f)


def site_parcels(nbhd):
    """Only parcels descended from our pipeline (BLOCK_/P95_ specs) --
    excludes the surrounding neighborhood context the baseline fixture
    ships with, which would otherwise swamp the render."""
    return [p for p in nbhd["parcels"] if (p.get("spec") or "").startswith(("BLOCK_", "P95_"))]


def extrude_polygon(outer_ring, holes, height, origin_lng, origin_lat):
    pts = ring_to_xy(outer_ring, origin_lng, origin_lat)
    if len(pts) < 3:
        return None
    wp = cq.Workplane("XY").polyline(pts).close()
    for hole in holes:
        hpts = ring_to_xy(hole, origin_lng, origin_lat)
        if len(hpts) >= 3:
            wp = wp.polyline(hpts).close()  # additional wire on same workplane -- treated as a hole/face by extrude when nested
    try:
        solid = wp.extrude(height)
        return solid
    except Exception as e:
        print(f"  ! skipped a polygon (extrude failed: {e})", file=sys.stderr)
        return None


def punch_openings(solid, building, origin_lng, origin_lat):
    """Cut `building`'s window/door openings (placed by
    `p221_natural_doors_and_windows`) out of `solid` via a real OpenCascade
    boolean subtraction -- this is what makes the isometric render show
    real openings instead of a blank wall.

    Each `Opening` references a wall edge by `ring_index`/`on_hole` into
    `building["polygon"]["outer"]` or `["holes"][0]` (the SAME rings the
    Rust operator indexed, using its own per-building local projection --
    `ring_index` is just an array index and `t` a fraction of that edge's
    own length, both invariant to which nearby origin the lng/lat->meters
    projection used, so reusing this scene's shared origin here is exact,
    not approximate).

    A punch is an axis-aligned box, oriented along the wall edge's own
    direction and centered at the opening's position, deep enough
    (`OPENING_PUNCH_DEPTH_M`) to fully pierce the mass -- this pipeline has
    no wall-thickness model, so "cut a hole all the way through a solid
    block" is the honest abstraction, not a real window reveal (P223 Deep
    Reveals, named but unverified -- see the operator's own module doc).

    All punches are subtracted in ONE `Shape.cut(*toCut)` call (OCC's
    native multi-tool boolean, not cadquery's usual single-tool
    `Workplane.cut`) -- pre-fusing hundreds of punches via pairwise
    `.union()` first, or cutting them one at a time, both measured multiple
    minutes on the largest P108-merged party-wall buildings (close to a
    thousand openings on one real, non-box extruded solid); the grouped
    multi-tool cut does the same ~1000-opening building in single-digit
    seconds.
    """
    openings = building.get("openings") or []
    if not openings:
        return solid

    outer_ring = ring_to_xy(building["polygon"]["outer"], origin_lng, origin_lat)
    holes = building["polygon"].get("holes") or []
    hole_ring = ring_to_xy(holes[0], origin_lng, origin_lat) if holes else None

    punch_solids = []
    for o in openings:
        ring = hole_ring if o.get("on_hole") else outer_ring
        if not ring:
            continue
        n = len(ring)
        i = o["ring_index"]
        if i >= n:
            continue
        ax, ay = ring[i]
        bx, by = ring[(i + 1) % n]
        edge_len = math.hypot(bx - ax, by - ay)
        if edge_len < 1e-6:
            continue
        t = o["t"]
        mx, my = ax + (bx - ax) * t, ay + (by - ay) * t
        angle_deg = math.degrees(math.atan2(by - ay, bx - ax))
        width = max(o["width_m"], 0.1)
        height = max(o["head_height_m"] - o["sill_height_m"], 0.1)
        z_bottom = o["floor"] * FLOOR_TO_FLOOR_M + o["sill_height_m"]
        try:
            punch = (
                cq.Workplane("XY")
                .box(width, OPENING_PUNCH_DEPTH_M, height)
                .rotate((0, 0, 0), (0, 0, 1), angle_deg)
                .translate((mx, my, z_bottom + height / 2))
            )
            punch_solids.append(punch.val())
        except Exception as e:
            print(f"  ! opening punch build failed: {e}", file=sys.stderr)

    if not punch_solids:
        return solid
    try:
        result_shape = solid.val().cut(*punch_solids)
        return cq.Workplane(obj=result_shape)
    except Exception as e:
        print(f"  ! opening cut failed for {building.get('id')}: {e}", file=sys.stderr)
        return solid


def shared_boundary(poly_a, poly_b, eps=0.05):
    """Find the coincident edge two adjacent `InteriorCell` polygons share
    -- both p127_intimacy_gradient's band/bay cuts and its loop-closing
    passage are built from the SAME shared threshold lines, so adjacent
    cells always have an exactly (up to floating point) coincident edge
    somewhere on their boundary. Matched by nearby endpoints, forward or
    reversed winding. Returns `((ax,ay),(bx,by))` or `None`.
    """
    def close(p, q):
        return math.hypot(p[0] - q[0], p[1] - q[1]) < eps

    na, nb = len(poly_a), len(poly_b)
    for i in range(na):
        a1, a2 = poly_a[i], poly_a[(i + 1) % na]
        for j in range(nb):
            b1, b2 = poly_b[j], poly_b[(j + 1) % nb]
            if (close(a1, b1) and close(a2, b2)) or (close(a1, b2) and close(a2, b1)):
                return (a1, a2)
    return None



def load_context_buildings(path, origin_lng, origin_lat):
    """Real surrounding-building massing from a pre-filtered Overture Maps
    GeoJSON extract (see `data/military-circle-context-buildings.geojson`'s
    own `_provenance` field) -- simple flat boxes at each footprint's real
    height (or `DEFAULT_CONTEXT_HEIGHT_M` where Overture has none), no
    window/door punching, no per-building fusion. These exist to place the
    generated site in its real neighborhood, not to be looked at closely --
    keeping them cheap matters: this codebase's own buildings already cost
    real cadquery/OpenCascade time per solid, and there are hundreds of
    these versus a few dozen of ours.

    The source file is expected to already exclude anything overlapping our
    own site (done once at data-prep time with shapely, not here -- see the
    file's own `_provenance.filtered` note) so this function does no
    intersection testing of its own.
    """
    with open(path) as f:
        geojson = json.load(f)
    solids = []
    for feat in geojson.get("features", []):
        geom = feat.get("geometry") or {}
        rings = []
        if geom.get("type") == "Polygon":
            rings = [geom["coordinates"][0]]
        elif geom.get("type") == "MultiPolygon":
            rings = [part[0] for part in geom["coordinates"]]
        for ring in rings:
            outer = [{"lng": lng, "lat": lat} for lng, lat in ring]
            area_m2 = polygon_area_m2(ring_to_xy(outer, origin_lng, origin_lat))
            if area_m2 < CONTEXT_MIN_AREA_M2:
                continue
            height = feat["properties"].get("height") or DEFAULT_CONTEXT_HEIGHT_M
            solid = extrude_polygon(outer, [], height, origin_lng, origin_lat)
            if solid is not None:
                solids.append((solid, "context"))
    return solids


def build_scene(nbhd, context_path=None):
    parcels = site_parcels(nbhd)
    if not parcels:
        raise SystemExit("no site parcels found -- check spec filtering")

    # Origin = centroid of all site parcel vertices.
    all_pts = [p for parcel in parcels for p in parcel["polygon"]["outer"]]
    origin_lng = sum(p["lng"] for p in all_pts) / len(all_pts)
    origin_lat = sum(p["lat"] for p in all_pts) / len(all_pts)

    building_solids = []  # (solid, color_name) -- real punched openings, for the isometric view
    building_solids_unpunched = []  # same footprints/heights, no window/door boolean cuts -- see export_glb's own docstring for why
    building_ids_with_real_shape = set()

    # Real P107-shaped buildings first (real height, may have a courtyard hole).
    # `polygon.get("parts")` is always a single element for buildings this
    # pipeline emits (P107 never produces a multi-part Building) -- opening
    # ring_index/on_hole reference the building's own top-level outer/holes,
    # so punch_openings assumes that single-part case; not handled in
    # general for a hypothetical multi-part building.
    for b in nbhd.get("buildings", []):
        height = b.get("height_m") or DEFAULT_BUILDING_HEIGHT_M
        parts = b["polygon"].get("parts") or [{"outer": b["polygon"]["outer"], "holes": b["polygon"].get("holes", [])}]
        for part in parts:
            solid = extrude_polygon(part["outer"], part.get("holes", []), height, origin_lng, origin_lat)
            if solid is not None:
                building_solids_unpunched.append((solid, "building_shaped"))
                punched = punch_openings(solid, b, origin_lng, origin_lat)
                building_solids.append((punched, "building_shaped"))
        # Track the pad id this building came from so we don't double-extrude it below.
        bid = b["id"]
        if bid.endswith("_building"):
            building_ids_with_real_shape.add(bid[: -len("_building")])

    # Un-shaped pads (P95 produced them, P107 didn't get to them) -- massing
    # box at P96's assigned height if it ran (target_stories * a 3.5m
    # floor-to-floor assumption, matching P107's own convention), else the
    # flat default, so the render isn't missing most of the actual building
    # count -- and doesn't understate height variation P96 actually assigned
    # just because P107 never got around to shaping the pad into a real
    # Building.
    for p in parcels:
        if p.get("use_category") not in ("p95_building_pad", "p95_pad_with_building"):
            continue
        if p["id"] in building_ids_with_real_shape:
            continue
        target_stories = p.get("target_stories")
        height = target_stories * FLOOR_TO_FLOOR_M if target_stories else DEFAULT_BUILDING_HEIGHT_M
        parts = p["polygon"].get("parts") or [{"outer": p["polygon"]["outer"], "holes": p["polygon"].get("holes", [])}]
        for part in parts:
            solid = extrude_polygon(part["outer"], part.get("holes", []), height, origin_lng, origin_lat)
            if solid is not None:
                building_solids.append((solid, "building_unshaped"))
                building_solids_unpunched.append((solid, "building_unshaped"))

    # Plazas / open space -- thin colored slabs at ground level.
    plaza_solids = []
    for o in nbhd.get("open_space", []):
        parts = o["polygon"].get("parts") or [{"outer": o["polygon"]["outer"], "holes": o["polygon"].get("holes", [])}]
        for part in parts:
            solid = extrude_polygon(part["outer"], part.get("holes", []), PLAZA_THICKNESS_M, origin_lng, origin_lat)
            if solid is not None:
                kind = o.get("kind") if o.get("kind") in ("undecided", "common") else "plaza"
                plaza_solids.append((solid, kind))

    # Streets -- thin ribbons along the centerline, buffered by row_width_m.
    street_solids = []
    for s in nbhd.get("streets", []):
        line = s.get("centerline", [])
        if len(line) < 2:
            continue
        width = (s.get("row_width_m") or 4.0) / 2.0
        pts = [project(p["lng"], p["lat"], origin_lng, origin_lat) for p in line]
        for a, b in zip(pts, pts[1:]):
            dx, dy = b[0] - a[0], b[1] - a[1]
            length = math.hypot(dx, dy)
            if length < 1e-6:
                continue
            nx, ny = -dy / length * width, dx / length * width
            corridor = [
                (a[0] + nx, a[1] + ny),
                (b[0] + nx, b[1] + ny),
                (b[0] - nx, b[1] - ny),
                (a[0] - nx, a[1] - ny),
            ]
            try:
                solid = cq.Workplane("XY").polyline(corridor).close().extrude(STREET_THICKNESS_M)
                street_solids.append((solid, s.get("classification") or "street"))
            except Exception:
                pass

    context_solids = load_context_buildings(context_path, origin_lng, origin_lat) if context_path else []

    return {
        "buildings": building_solids,
        "buildings_unpunched": building_solids_unpunched,
        "plazas": plaza_solids,
        "streets": street_solids,
        "context": context_solids,
        "origin": (origin_lng, origin_lat),
    }


# Lightened from the original near-black palette -- a flat #2b2620 fill
# with no shading reads as a solid silhouette with no readable form at
# real building-mass scale. Real per-face lighting (below) needs a base
# color with room to shade brighter/darker; near-black has nowhere to go.
COLORS = {
    "building_shaped": "#8a5a44",
    "building_unshaped": "#a3846a",
    "plaza": "#d9a441",
    "common": "#a3b18a",
    "undecided": "#b8602a",
    "local": "#6b6259",
    "pedestrian": "#9b8f7a",
    "street": "#6b6259",
    "context": "#5a5a5e",  # flat neutral gray -- deliberately duller than every
    # generated-site color so real Overture context reads as backdrop, not
    # competes with the pattern-language buildings it's surrounding.
}

# Two-light setup, not one. A single directional light leaves every face
# NOT facing it at flat ambient -- with a mostly-boxy massing scene that's
# roughly half the visible surfaces reading as a dark, formless silhouette.
# KEY is the main sun-like light (matches the isometric camera, elev=35,
# azim=-60, so the faces facing the viewer catch the strongest highlight).
# FILL comes from roughly the opposite side, weaker, so the shadow side
# still reveals real form (a wall, a corner, a courtyard notch) instead of
# going flat -- the same key+fill logic a real architectural render uses.
KEY_LIGHT_DIR = np.array([-0.45, -0.35, 0.82])
KEY_LIGHT_DIR = KEY_LIGHT_DIR / np.linalg.norm(KEY_LIGHT_DIR)
FILL_LIGHT_DIR = np.array([0.55, 0.45, 0.35])
FILL_LIGHT_DIR = FILL_LIGHT_DIR / np.linalg.norm(FILL_LIGHT_DIR)
AMBIENT = 0.28      # floor brightness even where neither light reaches
KEY_DIFFUSE = 0.55  # brightness added for faces facing the key light
FILL_DIFFUSE = 0.30  # brightness added for faces facing the fill light


def solid_to_triangles(solid):
    """Tessellate a cadquery solid into (vertices, triangle-index) via its
    underlying OCC shape."""
    shape = solid.val() if hasattr(solid, "val") else solid
    vertices, triangles = shape.tessellate(0.5)
    verts = np.array([(v.x, v.y, v.z) for v in vertices])
    tris = np.array(triangles)
    return verts, tris


def shade_faces(face_verts, base_hex, alpha):
    """Per-triangle Lambertian shading (key + fill) + translucency: one
    RGBA color per face, computed from that face's own normal against BOTH
    lights. This is what actually reveals building form (which walls face
    which light, which don't) instead of every face reading as the same
    flat silhouette color -- and the fill light means the shadow side
    still shows real form instead of going flat."""
    base_rgb = np.array(mcolors.to_rgb(base_hex))
    v0, v1, v2 = face_verts[:, 0], face_verts[:, 1], face_verts[:, 2]
    normals = np.cross(v1 - v0, v2 - v0)
    norms = np.linalg.norm(normals, axis=1, keepdims=True)
    norms[norms < 1e-9] = 1.0
    normals = normals / norms
    key = np.clip(normals @ KEY_LIGHT_DIR, 0.0, None)
    fill = np.clip(normals @ FILL_LIGHT_DIR, 0.0, None)
    intensity = AMBIENT + KEY_DIFFUSE * key + FILL_DIFFUSE * fill
    intensity = np.clip(intensity, 0.0, 1.15)  # small overshoot allowed for a real specular-ish highlight
    rgb = np.clip(base_rgb[None, :] * intensity[:, None], 0.0, 1.0)
    rgba = np.concatenate([rgb, np.full((len(rgb), 1), alpha)], axis=1)
    return rgba


def render_isometric(scene, out_path, title):
    fig = plt.figure(figsize=(10, 10))
    ax = fig.add_subplot(111, projection="3d")
    fig.patch.set_facecolor("#2a2a2e")
    ax.set_facecolor("#2a2a2e")

    def add_group(items, alpha):
        for solid, kind in items:
            try:
                verts, tris = solid_to_triangles(solid)
            except Exception as e:
                print(f"  ! tessellate failed: {e}", file=sys.stderr)
                continue
            face_verts = verts[tris]
            base_hex = COLORS.get(kind, "#999999")
            rgba = shade_faces(face_verts, base_hex, alpha)
            poly = Poly3DCollection(face_verts, facecolors=rgba, edgecolor="#f6f3ed22", linewidth=0.2)
            ax.add_collection3d(poly)

    # Real surrounding buildings first, most translucent of anything in the
    # scene -- backdrop, not subject. Camera framing below is deliberately
    # NOT widened to fit context (see load_context_buildings' own docstring):
    # it renders within whatever's already visible around the generated
    # site, cropped at the edges, rather than shrinking our own buildings to
    # fit a wider radius every time this scenario's baseline is compared.
    add_group(scene.get("context", []), alpha=0.45)

    # Translucent enough to read overlapping massing/depth, not so
    # translucent the scene turns to fog -- streets/plazas thinnest (they're
    # ground-plane slabs, least important to see "through"), buildings the
    # most opaque single layer but still see-through against neighbors.
    add_group(scene["streets"], alpha=0.55)
    add_group(scene["plazas"], alpha=0.6)
    add_group(scene["buildings"], alpha=0.82)

    all_solids = scene["streets"] + scene["plazas"] + scene["buildings"]
    all_verts = np.concatenate([solid_to_triangles(s)[0] for s, _ in all_solids if True], axis=0)
    xmin, ymin, zmin = all_verts.min(axis=0)
    xmax, ymax, zmax = all_verts.max(axis=0)
    max_range = max(xmax - xmin, ymax - ymin, 60) / 2
    cx, cy = (xmax + xmin) / 2, (ymax + ymin) / 2
    ax.set_xlim(cx - max_range, cx + max_range)
    ax.set_ylim(cy - max_range, cy + max_range)
    z_span = max(zmax, 20)
    ax.set_zlim(0, z_span)
    # A fixed z-aspect (this used to be a hardcoded 0.25) doesn't know how
    # wide the site actually is -- on a site whose xy footprint is much
    # larger than its building heights (the normal case: ~500m site,
    # ~15-20m buildings), 0.25 exaggerates height by 5-6x relative to true
    # scale, which is exactly what read as "weirdly tall" buildings that
    # were, in the real data, ordinary 4-story proportions. Compute the
    # TRUE proportional aspect from this scene's own real extents, then
    # apply a modest, fixed exaggeration (buildings still need to read as
    # taller-than-a-pancake at this zoomed-out a view) instead of a guess
    # that happened to fit one previous scene's proportions and not others.
    true_z_aspect = z_span / (2 * max_range)
    HEIGHT_EXAGGERATION = 2.5
    z_aspect = min(0.35, true_z_aspect * HEIGHT_EXAGGERATION)
    ax.set_box_aspect((1, 1, z_aspect))
    ax.view_init(elev=35, azim=-60)
    ax.set_axis_off()
    ax.set_title(title, fontsize=13, color="#f6f3ed")
    fig.tight_layout()
    fig.savefig(out_path, dpi=140, facecolor=fig.get_facecolor())
    plt.close(fig)
    print(f"wrote {out_path}")


def export_glb(scene, out_path):
    """Export the full scene as a single binary glTF (.glb) -- a real,
    colored 3D model any standard viewer can open (web three.js/
    `<model-viewer>`, Blender, VS Code's built-in 3D preview, online glTF
    viewers) directly, without re-running the Rust pipeline or cadquery to
    look at it again.

    Uses `scene["buildings_unpunched"]`, NOT `scene["buildings"]` -- unlike
    `render_isometric`, which just rasterizes whatever it's given into a
    fixed-size PNG no matter how complex the source mesh, cadquery's GLTF
    writer re-tessellates straight from the BREP geometry with (as far as
    this file's own testing found) no way to request a coarser mesh --
    `Assembly.save`'s own `tolerance`/`angularTolerance` kwargs, and
    manually calling `shape.tessellate()` at a coarser value before
    `asm.add()`, both produced byte-identical output regardless of the
    value passed. Measured directly, not assumed: `clean_baseline`'s 24
    buildings WITH `punch_openings`'s real window/door boolean cuts
    produced 145,344 triangles and a ~25.4 MiB file (real courtyard
    buildings with dozens of openings each -- every subtraction adds real
    topological complexity, and it compounds); the exact same 24
    buildings WITHOUT punching produced 956 triangles and 0.17 MiB.
    `scene["context"]` (261 real Overture buildings, plain unfused boxes,
    no instancing/geometry sharing across assembly members either) was
    ALSO excluded the first time this was built, on the assumption it was
    part of the size problem -- measuring it directly (below) showed it
    wasn't, so it's included now: real surrounding massing (the same
    Overture data `render_isometric` already draws for `clean_baseline`),
    so the interactive model shows the generated design sitting in its
    real site context too, not just floating in isolation.

    First found the hard way: 261 context buildings alone were blamed and
    excluded, but the resulting file was STILL ~26 MiB (over Cloudflare
    Workers' hard 25 MiB per-asset limit) -- which silently failed the
    Workers deploy step and skipped every step after it, including the
    actual Pages deploy that serves the live site, so a real code change
    shipped in CI green but never actually reached production. Fixed for
    real only after measuring where the triangles were actually coming
    from (punching, not context, and not context's ~3,100 simple box
    triangles either -- confirmed directly before re-adding it here, not
    assumed safe by analogy).

    The isometric PNG export keeps full punched detail (`render_isometric`
    still uses `scene["buildings"]`) -- only the downloadable/interactive
    .glb trades real window/door cuts for a plain massing model, to stay
    inside a hard external size limit rather than a soft budget. No interior walls
    either way -- see `render_floor_plan`'s own module doc for why
    interior partitions are drawn as a 2D plan instead of built into the
    3D solid.
    """
    asm = cq.Assembly()
    n_added = 0
    for group, alpha, solids_key in (
        ("streets", 0.55, "streets"), ("plazas", 0.6, "plazas"), ("buildings", 0.82, "buildings_unpunched"),
        ("context", 0.45, "context"),  # matches render_isometric's own context alpha -- backdrop, not the model
    ):
        for solid, kind in scene.get(solids_key, []):
            r, g, b = mcolors.to_rgb(COLORS.get(kind, "#999999"))
            n_added += 1
            try:
                asm.add(solid, name=f"{group}_{n_added}_{kind}", color=cq.Color(r, g, b, alpha))
            except Exception as e:
                print(f"  ! glb: skipped a {group} piece: {e}", file=sys.stderr)
    if n_added == 0:
        print(f"  ! nothing to export for {out_path}, skipping")
        return
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", FutureWarning)  # cadquery 2.8's Assembly.save, not our call
            asm.save(out_path, exportType="GLTF")
        print(f"wrote {out_path}")
    except Exception as e:
        print(f"  ! GLB export failed for {out_path}: {e}", file=sys.stderr)


def render_floor_plan(nbhd, origin_lng, origin_lat, floor, out_path, title):
    """Draw a real floor plan straight from `interior_cells` polygon data --
    plain 2D line art, no CSG.

    The first version of this tried to get there by unioning thin wall
    slabs into each building's extruded solid, then taking a horizontal
    section through the result (same technique `punch_openings` uses for
    exterior openings, just adding material instead of subtracting it).
    That doesn't work: the extruded solid has no wall thickness and no
    room voids -- it's a single filled block covering the whole footprint
    -- so a wall slab built inside it is (almost) entirely already inside
    existing material. Fusing it in changes essentially nothing, which is
    exactly what a direct diagnostic showed (comparing cross-section
    volume/face counts with and without the walls unioned in, on a real
    4-room chain from this fixture: the "extra" geometry the union added
    was a ~1% sliver from square-cut wall ends poking past an
    non-perpendicular building edge, not a room-dividing void). No
    rendering method was ever going to make that visible, because there
    was nothing there to see.

    `p127_intimacy_gradient`/`p129_common_areas_at_the_heart`/
    `p131_the_flow_through_rooms` already computed exactly what a floor
    plan needs -- each room's own footprint polygon, and which rooms
    connect through a door -- as 2D data. Drawing that directly is both
    correct (no solid-mass modeling assumption to get wrong) and, per the
    "how are we going from submillisecond plan calculations to a 20-minute
    render" complaint that started this detour, obviously the right
    complexity class: 2D polygon math, not a B-rep boolean.

    For each connected cell pair, the shared boundary edge
    (`shared_boundary`, the same helper the CSG version used to find it)
    is drawn as a wall line with a door-width gap at its midpoint. Ground
    floor only, since `interior_cells` is ground-floor-only (no staircase
    pattern exists yet to reach an upper one) -- a `floor` with no cells
    anywhere just draws every building's plain footprint outline, same as
    a massing box that never got `interior_cells` at all.
    """
    fig, plot_ax = plt.subplots(figsize=(12, 12))
    any_drawn = False
    for b in nbhd.get("buildings", []):
        outer_xy = ring_to_xy(b["polygon"]["outer"], origin_lng, origin_lat)
        if len(outer_xy) < 3:
            continue
        any_drawn = True
        loop = outer_xy + [outer_xy[0]]
        plot_ax.plot([p[0] for p in loop], [p[1] for p in loop], color="#2b2620", linewidth=1.4, zorder=2)
        for hole in b["polygon"].get("holes") or []:
            hole_xy = ring_to_xy(hole, origin_lng, origin_lat)
            if len(hole_xy) < 3:
                continue
            hole_loop = hole_xy + [hole_xy[0]]
            plot_ax.plot([p[0] for p in hole_loop], [p[1] for p in hole_loop], color="#2b2620", linewidth=1.4, zorder=2)

        cells = [c for c in (b.get("interior_cells") or []) if c.get("floor", 0) == floor]
        if len(cells) < 2:
            continue
        cell_xy = {c["id"]: ring_to_xy(c["polygon"]["outer"], origin_lng, origin_lat) for c in cells}
        seen_pairs = set()
        for c in cells:
            for other_id in c.get("connects_to") or []:
                pair = tuple(sorted((c["id"], other_id)))
                if pair in seen_pairs or other_id not in cell_xy:
                    continue
                seen_pairs.add(pair)
                edge = shared_boundary(cell_xy[c["id"]], cell_xy[other_id])
                if edge is None:
                    continue
                (ex1, ey1), (ex2, ey2) = edge
                length = math.hypot(ex2 - ex1, ey2 - ey1)
                if length < INTERIOR_WALL_MIN_LENGTH_M:
                    continue
                ux, uy = (ex2 - ex1) / length, (ey2 - ey1) / length
                mx, my = (ex1 + ex2) / 2, (ey1 + ey2) / 2
                half_door = min(INTERIOR_DOOR_WIDTH_M, length * 0.6) / 2
                gx1, gy1 = mx - ux * half_door, my - uy * half_door
                gx2, gy2 = mx + ux * half_door, my + uy * half_door
                plot_ax.plot([ex1, gx1], [ey1, gy1], color="#8a5a44", linewidth=1.0, zorder=1)
                plot_ax.plot([gx2, ex2], [gy2, ey2], color="#8a5a44", linewidth=1.0, zorder=1)

    if not any_drawn:
        print(f"  ! no buildings for {out_path}, skipping")
        plt.close(fig)
        return
    plot_ax.set_aspect("equal")
    plot_ax.set_title(title)
    plot_ax.axis("off")
    fig.tight_layout()
    fig.savefig(out_path, format="svg")
    plt.close(fig)
    print(f"wrote {out_path}")


WINDOW_COLOR = "#4f7d96"
COURTYARD_WINDOW_COLOR = "#7bafc4"
DOOR_COLOR = "#b8602a"
COMMON_AREA_MARKER_COLOR = "#2e6b4f"
# Neutral gray, deliberately outside the warm depth-gradient palette --
# p133_staircase_as_a_stage's stair core is circulation, not a step in the
# public/private sequence, and shouldn't read as one.
STAIR_FILL_COLOR = "#9a9690"

# p127_intimacy_gradient's public(0.0) -> private(1.0) depth, as a fill
# color -- cream at the public end, deep rust at the private end. Same
# warm family as WINDOW_COLOR/DOOR_COLOR/the interior-wall brown, so the
# gradient reads as part of one palette instead of a bolted-on heatmap.
DEPTH_COLOR_PUBLIC = (0.965, 0.933, 0.867)   # #f6eeDD
DEPTH_COLOR_PRIVATE = (0.494, 0.243, 0.145)  # #7e3e25


def depth_to_fill_color(depth):
    d = max(0.0, min(1.0, depth))
    r = DEPTH_COLOR_PUBLIC[0] + (DEPTH_COLOR_PRIVATE[0] - DEPTH_COLOR_PUBLIC[0]) * d
    g = DEPTH_COLOR_PUBLIC[1] + (DEPTH_COLOR_PRIVATE[1] - DEPTH_COLOR_PUBLIC[1]) * d
    b = DEPTH_COLOR_PUBLIC[2] + (DEPTH_COLOR_PRIVATE[2] - DEPTH_COLOR_PUBLIC[2]) * d
    return (r, g, b)


def polygon_area_m2(ring_xy):
    """Shoelace area of a ring already in local meters -- works whether
    `ring_xy` is closed (first == last, the wraparound term degenerates to
    0 and contributes nothing) or open (the convention `ring_to_xy`
    actually returns, having dropped the duplicate closing point: without
    explicit `% n` wraparound here, the missing closing edge silently
    undercounts -- caught by checking a real interior-cell quad's area
    against a hand-computed shoelace, not by the building-footprint-ranking
    use this was first written for, where the error is proportionally
    small enough on a many-sided polygon to not have changed which
    building ranked largest)."""
    n = len(ring_xy)
    if n < 3:
        return 0.0
    total = 0.0
    for i in range(n):
        x1, y1 = ring_xy[i]
        x2, y2 = ring_xy[(i + 1) % n]
        total += x1 * y2 - x2 * y1
    return abs(total) / 2.0


def nice_scale_bar_length_m(span_m):
    """Round scale-bar length (meters) close to 20% of `span_m`, from a
    fixed set of human-legible round numbers -- there is no in-frame
    dimension text anywhere else in this renderer, so without a scale bar
    a room fill has no way to tell a viewer whether it's closet-sized or
    ballroom-sized."""
    target = max(span_m * 0.2, 0.1)
    for candidate in (1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000):
        if candidate >= target:
            return candidate
    return 2000


def draw_scale_bar(ax, xlim, ylim):
    span_m = xlim[1] - xlim[0]
    bar_m = nice_scale_bar_length_m(span_m)
    x0 = xlim[0] + span_m * 0.06
    y0 = ylim[0] + (ylim[1] - ylim[0]) * 0.06
    tick_h = (ylim[1] - ylim[0]) * 0.012
    ax.plot([x0, x0 + bar_m], [y0, y0], color="#2b2620", linewidth=1.4, zorder=6, solid_capstyle="butt")
    for x in (x0, x0 + bar_m):
        ax.plot([x, x], [y0 - tick_h, y0 + tick_h], color="#2b2620", linewidth=1.4, zorder=6)
    ax.text(
        x0 + bar_m / 2, y0 + tick_h * 2.5, f"{bar_m} m",
        ha="center", va="bottom", fontsize=8, color="#2b2620", zorder=6,
    )


def render_largest_building_floors(nbhd, origin_lng, origin_lat, out_path):
    """Zoom into ONE building -- the largest footprint on the site -- and
    draw each of its floors as its own panel, side by side, all sharing one
    fixed extent so the panels are directly comparable.

    `render_floor_plan` draws every building's ground floor at once, at
    whole-site scale -- correct data, but at that zoom a building with
    several rooms reads as an illegible tangle of short wall segments (see
    that function's own module doc and the caveats already logged there).
    This is the zoomed-in complement: one building, filling the frame.

    Ground floor (floor 0) is the only one with real interior_cells data
    (no vertical-circulation pattern exists yet to place a staircase, so
    partitioning an upper floor would be fiction -- see
    `p127_intimacy_gradient`'s own module doc) -- its panel draws the real
    room graph the same way `render_floor_plan` does. Upper floors have no
    interior partition to draw, but DO have their own real, P221-placed
    window/door data (openings shrink with `size_falloff_per_floor` per
    floor) -- their panels draw the footprint outline plus that floor's own
    openings, so the comparison across floors is honest about what's real
    (the facade) and what isn't modeled yet (the upper-floor room layout),
    rather than just repeating the ground floor's partition upward.
    """
    buildings = nbhd.get("buildings", [])
    if not buildings:
        print(f"  ! no buildings for {out_path}, skipping")
        return

    def footprint_area(b):
        return polygon_area_m2(ring_to_xy(b["polygon"]["outer"], origin_lng, origin_lat))

    building = max(buildings, key=footprint_area)
    area_m2 = footprint_area(building)
    n_floors = max(building.get("floors") or 1, 1)

    outer_xy = ring_to_xy(building["polygon"]["outer"], origin_lng, origin_lat)
    if len(outer_xy) < 3:
        print(f"  ! largest building has a degenerate footprint, skipping {out_path}")
        return
    holes_xy = [ring_to_xy(h, origin_lng, origin_lat) for h in (building["polygon"].get("holes") or [])]

    xs = [p[0] for p in outer_xy]
    ys = [p[1] for p in outer_xy]
    pad = 3.0
    xlim = (min(xs) - pad, max(xs) + pad)
    ylim = (min(ys) - pad, max(ys) + pad)

    cells_by_floor = {}
    for c in building.get("interior_cells") or []:
        cells_by_floor.setdefault(c.get("floor", 0), []).append(c)
    openings_by_floor = {}
    for o in building.get("openings") or []:
        openings_by_floor.setdefault(o.get("floor", 0), []).append(o)

    fig, axes = plt.subplots(1, n_floors, figsize=(5.0 * n_floors, 5.5), squeeze=False)
    axes = axes[0]

    for floor in range(n_floors):
        ax = axes[floor]
        loop = outer_xy + [outer_xy[0]]
        ax.plot([p[0] for p in loop], [p[1] for p in loop], color="#2b2620", linewidth=1.6, zorder=2)
        for hole_xy in holes_xy:
            if len(hole_xy) < 3:
                continue
            hloop = hole_xy + [hole_xy[0]]
            ax.plot([p[0] for p in hloop], [p[1] for p in hloop], color="#2b2620", linewidth=1.6, zorder=2)

        cells = cells_by_floor.get(floor, [])
        if len(cells) >= 2:
            cell_xy = {c["id"]: ring_to_xy(c["polygon"]["outer"], origin_lng, origin_lat) for c in cells}

            # Privacy-gradient fill: p127_intimacy_gradient's own depth
            # (0.0 = public/entrance, 1.0 = deepest/private), one solid
            # fill per cell -- without this, room SIZE and the gradient's
            # actual shape are invisible; only wall lines drew before.
            # Also marks the cell p129_common_areas_at_the_heart flagged
            # is_common, since "which room is the shared heart" is the
            # other half of what these wall lines alone don't show.
            for c in cells:
                pts = cell_xy[c["id"]]
                kind = c.get("kind", "room")
                # p133_staircase_as_a_stage's stair core is circulation, not
                # a step in the public/private gradient -- its own depth
                # value is just copied from the common cell it was carved
                # out of, so filling it by depth would misleadingly place
                # it somewhere on the gradient it was never part of.
                fill_color = STAIR_FILL_COLOR if kind == "stair" else depth_to_fill_color(c.get("depth", 0.0))
                ax.fill(
                    [p[0] for p in pts], [p[1] for p in pts],
                    color=fill_color,
                    edgecolor="none", zorder=0,
                )
                ccx = sum(p[0] for p in pts) / len(pts)
                ccy = sum(p[1] for p in pts) / len(pts)
                if c.get("is_common"):
                    ax.plot(
                        [ccx], [ccy], marker="o", markersize=5,
                        markerfacecolor=COMMON_AREA_MARKER_COLOR,
                        markeredgecolor="none", zorder=4,
                    )
                if kind in ("entrance", "stair"):
                    ax.text(
                        ccx, ccy, kind, ha="center", va="center", fontsize=6.5,
                        color="#2b2620", zorder=5,
                        bbox=dict(boxstyle="round,pad=0.15", facecolor="white", edgecolor="none", alpha=0.75),
                    )

            seen_pairs = set()
            for c in cells:
                for other_id in c.get("connects_to") or []:
                    pair = tuple(sorted((c["id"], other_id)))
                    if pair in seen_pairs or other_id not in cell_xy:
                        continue
                    seen_pairs.add(pair)
                    edge = shared_boundary(cell_xy[c["id"]], cell_xy[other_id])
                    if edge is None:
                        continue
                    (ex1, ey1), (ex2, ey2) = edge
                    length = math.hypot(ex2 - ex1, ey2 - ey1)
                    if length < INTERIOR_WALL_MIN_LENGTH_M:
                        continue
                    ux, uy = (ex2 - ex1) / length, (ey2 - ey1) / length
                    mx, my = (ex1 + ex2) / 2, (ey1 + ey2) / 2
                    half_door = min(INTERIOR_DOOR_WIDTH_M, length * 0.6) / 2
                    gx1, gy1 = mx - ux * half_door, my - uy * half_door
                    gx2, gy2 = mx + ux * half_door, my + uy * half_door
                    ax.plot([ex1, gx1], [ey1, gy1], color="#8a5a44", linewidth=1.2, zorder=1)
                    ax.plot([gx2, ex2], [gy2, ey2], color="#8a5a44", linewidth=1.2, zorder=1)

        for o in openings_by_floor.get(floor, []):
            ring = holes_xy[0] if (o.get("on_hole") and holes_xy) else outer_xy
            n = len(ring)
            i = o["ring_index"]
            if n < 2 or i >= n:
                continue
            ax1, ay1 = ring[i]
            bx1, by1 = ring[(i + 1) % n]
            edge_len = math.hypot(bx1 - ax1, by1 - ay1)
            if edge_len < 1e-6:
                continue
            t = o["t"]
            mx, my = ax1 + (bx1 - ax1) * t, ay1 + (by1 - ay1) * t
            ux, uy = (bx1 - ax1) / edge_len, (by1 - ay1) / edge_len
            half_w = max(o["width_m"], 0.3) / 2
            p1 = (mx - ux * half_w, my - uy * half_w)
            p2 = (mx + ux * half_w, my + uy * half_w)
            is_door = o["kind"] == "door"
            if is_door:
                color = DOOR_COLOR
            else:
                # Distinct from WINDOW_COLOR by more than just meaning:
                # matplotlib/Agg silently drops some paths when very many
                # same-colored thin Line2D artists are interspersed with a
                # different color on one axes (confirmed by bisection --
                # street-facing and courtyard-facing windows painted the
                # SAME hex color made roughly half of a dense ring's
                # windows vanish from the render, even in a direct PNG
                # save with no SVG involved; splitting the color fixed it
                # outright). Real bug, real fix, and also a real
                # distinction worth drawing: which wall a window faces.
                color = COURTYARD_WINDOW_COLOR if o.get("on_hole") else WINDOW_COLOR
            ax.plot(
                [p1[0], p2[0]], [p1[1], p2[1]],
                color=color,
                linewidth=2.4 if is_door else 1.8,
                solid_capstyle="butt", zorder=3,
            )

        ax.set_aspect("equal")
        ax.set_xlim(*xlim)
        ax.set_ylim(*ylim)
        ax.axis("off")
        draw_scale_bar(ax, xlim, ylim)
        label = "ground floor" if floor == 0 else f"floor {floor + 1}"
        n_windows = sum(1 for o in openings_by_floor.get(floor, []) if o["kind"] == "window")
        n_doors = sum(1 for o in openings_by_floor.get(floor, []) if o["kind"] == "door")
        title = f"{label}\n{n_windows} windows, {n_doors} doors"
        if len(cells) >= 2:
            # Real dimensions, not just a wall diagram -- band_depth_m
            # slices by constant arc length, so these read close to a
            # single number; that sameness IS the finding, not a display
            # bug, and this makes it a checkable fact instead of an
            # impression from squinting at wall spacing.
            cell_areas = [polygon_area_m2(cell_xy[c["id"]]) for c in cells]
            title += (
                f"\n{len(cells)} rooms, {min(cell_areas):.0f}-{max(cell_areas):.0f} m² "
                f"(avg {sum(cell_areas) / len(cell_areas):.0f} m²)"
            )
        ax.set_title(title, fontsize=10)

    fig.suptitle(
        f"{building['id']}  ({area_m2:.0f} m² footprint, {n_floors} floor{'s' if n_floors != 1 else ''})",
        fontsize=12,
    )
    legend_handles = [
        plt.Line2D([0], [0], color="#2b2620", linewidth=1.6, label="exterior wall"),
        plt.Line2D([0], [0], color="#8a5a44", linewidth=1.2, label="interior partition (ground floor only)"),
        plt.Line2D([0], [0], color=WINDOW_COLOR, linewidth=1.8, label="window (street/yard-facing)"),
        plt.Line2D([0], [0], color=COURTYARD_WINDOW_COLOR, linewidth=1.8, label="window (courtyard-facing)"),
        plt.Line2D([0], [0], color=DOOR_COLOR, linewidth=2.4, label="door"),
        plt.Line2D(
            [0], [0], marker="o", linestyle="none", markersize=6,
            markerfacecolor=COMMON_AREA_MARKER_COLOR, markeredgecolor="none",
            label="common area (P129)",
        ),
        Patch(facecolor=depth_to_fill_color(0.0), edgecolor="none", label="public (depth 0, \"entrance\" cell)"),
        Patch(facecolor=depth_to_fill_color(1.0), edgecolor="none", label="private (deepest, depth 1)"),
        Patch(facecolor=STAIR_FILL_COLOR, edgecolor="none", label="stair core (P133)"),
    ]
    fig.legend(handles=legend_handles, loc="lower center", ncol=4, fontsize=9, frameon=False)
    fig.tight_layout(rect=(0, 0.05, 1, 0.95))
    fig.savefig(out_path, format="svg")
    plt.close(fig)
    print(f"wrote {out_path}")


def main():
    if len(sys.argv) not in (3, 4):
        print("usage: render.py <neighborhood.json> <output_prefix> [context_buildings.geojson]")
        sys.exit(1)
    nbhd_path, out_prefix = sys.argv[1], sys.argv[2]
    context_path = sys.argv[3] if len(sys.argv) == 4 else None
    nbhd = load(nbhd_path)
    print(f"=== {nbhd_path} ===")
    scene = build_scene(nbhd, context_path=context_path)
    print(
        f"buildings: {len(scene['buildings'])}, plazas: {len(scene['plazas'])}, "
        f"streets: {len(scene['streets'])}, context: {len(scene['context'])}"
    )

    render_isometric(scene, f"{out_prefix}_isometric.png", nbhd_path.split("/")[-1])

    origin_lng, origin_lat = scene["origin"]
    render_floor_plan(nbhd, origin_lng, origin_lat, 0, f"{out_prefix}_floorplan_ground.svg", "floor plan (ground)")
    max_floors = max((b.get("floors") or 1) for b in nbhd.get("buildings", [])) if nbhd.get("buildings") else 1
    if max_floors >= 2:
        render_floor_plan(
            nbhd, origin_lng, origin_lat, 1, f"{out_prefix}_floorplan_upper.svg", "floor plan (floor 2)"
        )
    render_largest_building_floors(nbhd, origin_lng, origin_lat, f"{out_prefix}_floorplan_largest_building.svg")

    export_glb(scene, f"{out_prefix}.glb")


if __name__ == "__main__":
    main()
