#!/usr/bin/env python3
"""Extrude a street-smarts Neighborhood JSON into real 3D solids (via
cadquery/OpenCascade -- the same B-rep kernel FreeCAD is built on; FreeCAD
itself isn't installable in this environment) and render wireframe plan,
elevation, floor-plan, and isometric views. Still a gut check on scale and
density, not a finished architectural rendering -- but window and door
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
    boolean subtraction -- this is what makes the elevation/plan exports
    show real openings instead of a blank wall.

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


def fuse_all(solids):
    """Combine `solids` (each a Workplane, or a `(Workplane, label)` tuple
    -- the shape kept, the label ignored) into one shape via ONE grouped
    OCC boolean fuse -- the same stage-everything-then-apply-once pattern
    as `punch_openings`' multi-tool cut, instead of N pairwise
    `.union()` calls each paying its own full boolean-op cost. Falls back
    to the old pairwise approach (skipping any one degenerate piece rather
    than failing the whole export) only if the grouped call itself throws
    -- a multi-tool BOP is usually MORE robust than a chain of pairwise
    ones, not less, but this keeps the original resilience as a backstop.
    """
    items = [s[0] if isinstance(s, tuple) else s for s in solids]
    if not items:
        return None
    if len(items) == 1:
        return items[0]
    try:
        fused_shape = items[0].val().fuse(*[s.val() for s in items[1:]])
        return cq.Workplane(obj=fused_shape)
    except Exception as e:
        print(f"  ! grouped fuse failed ({e}), falling back to pairwise union", file=sys.stderr)
        combined = items[0]
        for s in items[1:]:
            try:
                combined = combined.union(s)
            except Exception:
                pass  # keep going even if a union fails on a degenerate piece
        return combined


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



def build_scene(nbhd):
    parcels = site_parcels(nbhd)
    if not parcels:
        raise SystemExit("no site parcels found -- check spec filtering")

    # Origin = centroid of all site parcel vertices.
    all_pts = [p for parcel in parcels for p in parcel["polygon"]["outer"]]
    origin_lng = sum(p["lng"] for p in all_pts) / len(all_pts)
    origin_lat = sum(p["lat"] for p in all_pts) / len(all_pts)

    building_solids = []  # (solid, color_name)
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
                solid = punch_openings(solid, b, origin_lng, origin_lat)
                building_solids.append((solid, "building_shaped"))
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

    return {
        "buildings": building_solids,
        "plazas": plaza_solids,
        "streets": street_solids,
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


def build_exterior_master(scene):
    """Fuse every solid (streets + plazas + buildings) into ONE shape,
    ONCE. `render_svg_projection` used to redo
    this fuse from scratch on every call -- three full-scene fuses (plan,
    elevation front, elevation side) each paying the ~130-object BOP cost
    independently, when the underlying geometry never changes between
    them. Build once here, reuse for every flat projection."""
    all_solids = scene["streets"] + scene["plazas"] + scene["buildings"]
    if not all_solids:
        return None
    return fuse_all(all_solids)


def render_svg_projection(combined, out_path, direction, title):
    """Export a hidden-line SVG projection of an ALREADY-fused shape (see
    `build_exterior_master`) along `direction`. Takes the fused shape
    directly, not a scene, so the same master model can back every flat
    view without re-fusing."""
    if combined is None:
        print(f"  ! no solids for {out_path}, skipping")
        return
    try:
        cq.exporters.export(
            combined, out_path,
            opt={"projectionDir": direction, "showHidden": False},
        )
        print(f"wrote {out_path}")
    except Exception as e:
        print(f"  ! SVG export failed for {out_path}: {e}", file=sys.stderr)


def export_glb(scene, out_path):
    """Export the full scene as a single binary glTF (.glb) -- colored the
    same as `render_isometric`, window/door openings already cut. A real,
    colored 3D model any standard viewer can open (web three.js/
    `<model-viewer>`, Blender, VS Code's built-in 3D preview, online glTF
    viewers) directly, without re-running the Rust pipeline or cadquery to
    look at it again.

    Real punched exterior openings, no interior walls -- see
    `render_floor_plan`'s own module doc for why interior partitions are
    drawn as a 2D plan instead of built into the 3D solid.
    """
    asm = cq.Assembly()
    n_added = 0
    for group, alpha, solids_key in (
        ("streets", 0.55, "streets"), ("plazas", 0.6, "plazas"), ("buildings", 0.82, "buildings")
    ):
        for solid, kind in scene[solids_key]:
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


def main():
    if len(sys.argv) != 3:
        print("usage: render.py <neighborhood.json> <output_prefix>")
        sys.exit(1)
    nbhd_path, out_prefix = sys.argv[1], sys.argv[2]
    nbhd = load(nbhd_path)
    print(f"=== {nbhd_path} ===")
    scene = build_scene(nbhd)
    print(f"buildings: {len(scene['buildings'])}, plazas: {len(scene['plazas'])}, streets: {len(scene['streets'])}")

    render_isometric(scene, f"{out_prefix}_isometric.png", nbhd_path.split("/")[-1])

    exterior_master = build_exterior_master(scene)
    render_svg_projection(exterior_master, f"{out_prefix}_plan.svg", (0, 0, 1), "plan")
    render_svg_projection(exterior_master, f"{out_prefix}_elevation_front.svg", (0, -1, 0), "elevation (front)")
    render_svg_projection(exterior_master, f"{out_prefix}_elevation_side.svg", (1, 0, 0), "elevation (side)")

    origin_lng, origin_lat = scene["origin"]
    render_floor_plan(nbhd, origin_lng, origin_lat, 0, f"{out_prefix}_floorplan_ground.svg", "floor plan (ground)")
    max_floors = max((b.get("floors") or 1) for b in nbhd.get("buildings", [])) if nbhd.get("buildings") else 1
    if max_floors >= 2:
        render_floor_plan(
            nbhd, origin_lng, origin_lat, 1, f"{out_prefix}_floorplan_upper.svg", "floor plan (floor 2)"
        )

    export_glb(scene, f"{out_prefix}.glb")


if __name__ == "__main__":
    main()
