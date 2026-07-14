#!/usr/bin/env python3
"""Extrude a street-smarts Neighborhood JSON into real 3D solids (via
cadquery/OpenCascade -- the same B-rep kernel FreeCAD is built on; FreeCAD
itself isn't installable in this environment) and render wireframe plan,
elevation, and an isometric massing view. Just a gut check on scale and
density, not an architectural rendering -- massing blocks only, no
windows/doors/roofs, matching this project's own "abstract polygon, not a
real building design" caveats.

Input is the JSON a pattern pipeline run produces -- see
`crates/street-smarts-patterns/examples/dump_pipeline.rs`, or
`scripts/vibe-render.sh` for the end-to-end orchestration (Rust dump ->
this script) across both baseline scenarios.
"""
import json
import math
import sys

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
    for b in nbhd.get("buildings", []):
        height = b.get("height_m") or DEFAULT_BUILDING_HEIGHT_M
        parts = b["polygon"].get("parts") or [{"outer": b["polygon"]["outer"], "holes": b["polygon"].get("holes", [])}]
        for part in parts:
            solid = extrude_polygon(part["outer"], part.get("holes", []), height, origin_lng, origin_lat)
            if solid is not None:
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
    FLOOR_TO_FLOOR_M = 3.5
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

# Directional light, roughly matching the isometric camera (elev=35,
# azim=-60) so faces facing the viewer catch real highlight -- upper-left-
# front, slightly favoring +z so roofs read brightest, same as a real sun
# angle in an architectural massing render.
LIGHT_DIR = np.array([-0.45, -0.35, 0.82])
LIGHT_DIR = LIGHT_DIR / np.linalg.norm(LIGHT_DIR)
AMBIENT = 0.40   # floor brightness even for faces facing away from the light
DIFFUSE = 0.65   # additional brightness for faces facing the light head-on


def solid_to_triangles(solid):
    """Tessellate a cadquery solid into (vertices, triangle-index) via its
    underlying OCC shape."""
    shape = solid.val() if hasattr(solid, "val") else solid
    vertices, triangles = shape.tessellate(0.5)
    verts = np.array([(v.x, v.y, v.z) for v in vertices])
    tris = np.array(triangles)
    return verts, tris


def shade_faces(face_verts, base_hex, alpha):
    """Per-triangle Lambertian shading + translucency: one RGBA color per
    face, computed from that face's own normal against LIGHT_DIR. This is
    what actually reveals building form (which walls face the light, which
    don't) instead of every face reading as the same flat silhouette
    color."""
    base_rgb = np.array(mcolors.to_rgb(base_hex))
    v0, v1, v2 = face_verts[:, 0], face_verts[:, 1], face_verts[:, 2]
    normals = np.cross(v1 - v0, v2 - v0)
    norms = np.linalg.norm(normals, axis=1, keepdims=True)
    norms[norms < 1e-9] = 1.0
    normals = normals / norms
    intensity = AMBIENT + DIFFUSE * np.clip(normals @ LIGHT_DIR, 0.0, None)
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
    ax.set_zlim(0, max(zmax, 20))
    ax.set_box_aspect((1, 1, 0.25))
    ax.view_init(elev=35, azim=-60)
    ax.set_axis_off()
    ax.set_title(title, fontsize=13, color="#f6f3ed")
    fig.tight_layout()
    fig.savefig(out_path, dpi=140, facecolor=fig.get_facecolor())
    plt.close(fig)
    print(f"wrote {out_path}")


def render_svg_projection(scene, out_path, direction, title):
    all_solids = scene["streets"] + scene["plazas"] + scene["buildings"]
    if not all_solids:
        print(f"  ! no solids for {out_path}, skipping")
        return
    combined = all_solids[0][0]
    for s, _ in all_solids[1:]:
        try:
            combined = combined.union(s)
        except Exception:
            pass  # keep going even if a union fails on a degenerate piece
    try:
        cq.exporters.export(
            combined, out_path,
            opt={"projectionDir": direction, "showHidden": False},
        )
        print(f"wrote {out_path}")
    except Exception as e:
        print(f"  ! SVG export failed for {out_path}: {e}", file=sys.stderr)


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
    render_svg_projection(scene, f"{out_prefix}_plan.svg", (0, 0, 1), "plan")
    render_svg_projection(scene, f"{out_prefix}_elevation_front.svg", (0, -1, 0), "elevation (front)")
    render_svg_projection(scene, f"{out_prefix}_elevation_side.svg", (1, 0, 0), "elevation (side)")


if __name__ == "__main__":
    main()
