#!/usr/bin/env python3
"""Extrude a street-smarts Neighborhood JSON into real 3D solids (via
cadquery/OpenCascade -- the same B-rep kernel FreeCAD is built on; FreeCAD
itself isn't installable in this environment) and render floor-plan and
isometric views, plus a `.glb` for interactive viewing. Still a gut check
on scale and density, not a finished architectural rendering.

Window/door openings (`opening_records`/`opening_placement`, driven by
`p221_natural_doors_and_windows`'s real pattern-derived placement) are
FLAT DECALS now for the isometric PNG, not a real OpenCascade boolean cut
-- a real, measured architecture change, not a cosmetic one. The original
version cut a deep box out of each building's solid mass per opening
(`punch_openings`, removed); measured directly on the real
`clean_baseline` scenario, that boolean work was 61.93s of a 103.9s total
render (~60%) -- by a wide margin the single most expensive thing this
file did, for detail that only ever reached the flat isometric PNG anyway
(the GLB always had to throw punched solids away and fall back to plain
massing to stay under Cloudflare Workers' 25 MiB per-asset limit: 145,344
triangles / ~25.4 MiB punched vs 956 triangles / 0.17 MiB unpunched, on
the same 24 buildings). Replacing the cut with a thin, separately-colored
QUAD (window/courtyard-window/door, matching
`render_largest_building_floors`'s own 2D convention), drawn with plain
numpy/matplotlib rather than a cadquery/OCC solid, removes the boolean
entirely and collapses the isometric path's triangle count to roughly the
unpunched number.

The GLB stays exactly what it was before this change -- plain massing, no
window/door detail -- on purpose, not by oversight: giving it the SAME
decal detail was tried and measured (see `export_glb`'s own docstring),
and even a thin box per opening, merged into one compound per color to
rule out per-node overhead, still produced a ~28.5 MiB file (over budget
again) at this fixture's real 7,765-opening count. Real per-opening 3D
geometry is too much data for a 25 MiB budget regardless of how it's
packaged; a flat quad is only free because the isometric path never turns
it into a cadquery mesh at all. What's still NOT here: real wall
thickness on the exterior walls (a decal sits proud of a zero-thickness
wall, not in a real reveal).

Roof forms exist now for real, for the P117 Sheltering Roof / P162 North
Face slice specifically: `roof_cap_solid` builds a real triangular-wedge
shed roof over each building with a real `Building.roof` (a plain
EXTRUSION of a 2D triangular cross-section, not a boolean -- see its own
docstring for why that's the cheap primitive here, the mirror image of
the openings lesson above). Measured directly on the real `clean_baseline`
scenario: +0.18s of build_scene's own ~1.0s (real, bounded, not
per-opening-scaled), total render time 3.8s -> 4.6s, `.glb` size
694,724 -> 795,004 bytes -- still far under the 25 MiB budget. P116
Cascade of Roofs' own real per-wing cascade, P118 Roof Garden, and P119
Arcades/P166 Gallery Surround's own canopy geometry are NOT built here --
see `p117_sheltering_roof.rs`'s own module doc for why those stay
deferred, real gaps.

Massing is still, by default, one footprint swept straight up by one
height per building -- a real EXTRUSION, not per-floor VOLUMES. The one
exception is P124 Activity Pockets: `build_scene`'s own per-building
cutback step (see its comment there and `find_pocket_refill`'s own
docstring) cuts the bump's own footprint back out above ground level so
a pocket reads as a ground-floor nook projecting from the building, not
a floor-to-roof bay window -- a targeted fix for one real feature, not a
general per-floor footprint model.

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
as the isometric render's building massing (no window/door decals -- see
above) -- drop it into any standard glTF viewer (web three.js/
`<model-viewer>`, Blender, VS Code's 3D preview, an online glTF viewer)
directly, without re-running this pipeline just to look at the model
again.

Input is the JSON a pattern pipeline run produces -- see
`crates/street-smarts-patterns/examples/dump_pipeline.rs`, or
`scripts/vibe-render.sh` for the end-to-end orchestration (Rust dump ->
this script) across both baseline scenarios.
"""
import json
import math
import sys
import time
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
OPENING_DECAL_OFFSET_M = 0.03  # how far a window/door decal sits proud of
# the wall's own outward face -- enough to avoid z-fighting the wall's own
# coplanar surface at render/tessellation precision, small enough to still
# read as flush, not a separate floating panel. See opening_placement's
# own docstring for why this replaced a real OpenCascade boolean cut, and
# export_glb's own docstring for why this decal is isometric-PNG-only (a
# numpy quad, no cadquery/OCC solid involved) -- real per-opening 3D
# geometry, even just a thin box, is too much data for the GLB's 25 MiB
# budget at this fixture's real opening density (measured: 7,765 openings
# -> ~28.5 MiB either way).
INTERIOR_DOOR_WIDTH_M = 0.9  # floor-plan door-gap width, drawn in-plane -- no wall thickness/height in a 2D plan
INTERIOR_WALL_MIN_LENGTH_M = 1.2  # shorter than this + a door gap leaves no real wall -- skip it
DEFAULT_CONTEXT_HEIGHT_M = 6.0  # ~2 stories, for context buildings Overture has no height for
CONTEXT_MIN_AREA_M2 = 8.0  # drop slivers/artifacts smaller than a garden shed
POCKET_MATCH_EPS_M = 0.05  # vertex-join tolerance for find_pocket_refill --
# generous vs float noise from two independent local->lnglat conversions of
# the SAME Rust f64s (the join is exact in principle, see that function's
# own docstring), tiny vs any real building dimension.
ACTIVITY_MARKER_RADIUS_M = 0.6  # a small post, not a building -- real
# ActivityNode data has a point location and a kind, nothing else
# geometric (no footprint, no height); this is a rendering-layer choice
# of HOW to show a point, not a value read from the pipeline.
ACTIVITY_MARKER_HEIGHT_M = 2.5

# Window/door colors -- defined here (not just down by render_largest_building_floors,
# where they originated) so build_scene's own opening-decal path and the 2D
# floor-plan path share the SAME real colors instead of two independently
# maintained hex literals drifting apart.
WINDOW_COLOR = "#4f7d96"
COURTYARD_WINDOW_COLOR = "#7bafc4"
DOOR_COLOR = "#b8602a"


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


def roof_cap_solid(outer_ring, eave_height_m, ridge_height_m, origin_lng, origin_lat):
    """A real shed-roof cap for `p117_sheltering_roof`'s own `RoofForm`
    (`crates/street-smarts-core/src/nir.rs`) -- always `slope_azimuth_deg
    == 0.0` (true north) today, so this only ever builds a north-low,
    south-high slope; a real general-azimuth version isn't built yet since
    nothing produces any other bearing.

    A real triangular-wedge EXTRUSION, not a boolean: the roof's own 2D
    cross-section in the north-south/vertical (Y-Z) plane is a triangle --
    (south, ridge_height_m), (south, eave_height_m), (north, eave_height_m)
    -- meeting the wall top exactly at the low (north) eave and rising to
    the ridge at the south edge, extruded along the east-west axis. Same
    cheap primitive every wall extrusion already uses (`extrude_polygon`),
    not a `.cut()`/`.union()` -- this pipeline's own real, measured lesson
    (see this file's own module doc) is that a real boolean per building,
    multiplied across a real fixture, is what actually costs real render
    time; a plain extrusion doesn't carry that cost.

    Approximates the building's own real footprint by its real, true-
    north-aligned bounding box for the roof cap specifically (the wall
    extrusion below it still uses the EXACT real footprint, unchanged) --
    an honest simplification, not hidden: building a roof cap that follows
    a real non-rectangular footprint's own exact outline needs a genuinely
    non-planar ruled surface, a real, larger lift deferred along with P116
    Cascade of Roofs' own per-wing segments (see p117_sheltering_roof.rs's
    own module doc). `None` if the footprint or the real eave-to-ridge
    rise is degenerate.
    """
    pts = ring_to_xy(outer_ring, origin_lng, origin_lat)
    if len(pts) < 3:
        return None
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    x_min, x_max = min(xs), max(xs)
    y_min, y_max = min(ys), max(ys)  # true north = +y; y_min = south (ridge), y_max = north (eave)
    if x_max - x_min < 1e-6 or y_max - y_min < 1e-6 or ridge_height_m <= eave_height_m:
        return None
    try:
        profile = cq.Workplane("YZ").polyline(
            [(y_min, ridge_height_m), (y_min, eave_height_m), (y_max, eave_height_m)]
        ).close()
        solid = profile.extrude(x_max - x_min).translate((x_min, 0, 0))
        return solid
    except Exception as e:
        print(f"  ! skipped a roof cap (extrude failed: {e})", file=sys.stderr)
        return None


INTERIOR_WALL_HEIGHT_M = 2.7  # a plausible real interior ceiling height --
# NOT Alexander's own literal figure (p127_intimacy_gradient's own cells
# carry no height data at all), deliberately well under FLOOR_TO_FLOOR_M
# (3.5m) so a partition wall reads as an interior wall, not a second
# full-height exterior wall -- same "plausible, honestly labeled, not a
# cited number" category as p95_building_complex's own pad_inset_m.
MIN_INTERIOR_WALL_THICKNESS_M = 0.12  # only used as a fallback -- see
# interior_wall_thickness_for's own docstring for the real, preferred path.
FLOOR_PLATE_THICKNESS_M = 0.2  # a plausible real floor/ceiling slab depth
# (thin, no waffle/beam detail modeled) -- same "plausible, honestly
# labeled" category as the constants above, not a cited figure.


def interior_wall_thickness_for(building):
    """Real interior partition thickness for one building -- half its own
    real P197 `wall_thickness_m` (a plausible architectural convention: an
    interior partition doesn't carry the insulation/weatherproofing layers
    a full exterior wall assembly does, so roughly half that assembly's
    depth is a reasonable real figure -- p197_thick_walls' own module doc
    already says the same thing about ITS number: a plausible construction
    figure, not Alexander's own cited dimension either way). Falls back to
    `MIN_INTERIOR_WALL_THICKNESS_M` only when `p197_thick_walls` never ran
    on this real building (`wall_thickness_m` is `None`) -- real per-
    building data always wins when it exists.
    """
    wt = building.get("wall_thickness_m")
    if wt:
        return max(MIN_INTERIOR_WALL_THICKNESS_M, wt * 0.5)
    return MIN_INTERIOR_WALL_THICKNESS_M


def interior_partition_solids(cell_ring_xy, wall_height_m=INTERIOR_WALL_HEIGHT_M,
                               wall_thickness_m=MIN_INTERIOR_WALL_THICKNESS_M, z_offset_m=0.0):
    """Real, additive thin wall slabs along ONE `InteriorCell`'s own real
    polygon boundary (`p127_intimacy_gradient`'s own real depth-ordered
    partition) -- one real EXTRUSION per edge, no boolean, the same
    additive technique already used for the roof cap and (in the Rust
    generator itself) P124's bump.

    `InteriorCell.floor` is hard-coded 0 everywhere in this schema (see
    its own doc comment) -- there's no real per-floor room program to
    read, only this one real ground-floor layout. `z_offset_m` is how
    `build_scene` repeats this SAME real layout at every real floor level
    of the building it belongs to (see `build_scene`'s own docstring for
    why that's the honest treatment of a real gap, not fabrication): a
    non-zero value here doesn't mean a different real floor plan exists at
    that height, just that this call is placing the one real layout there.

    Adjacent cells sharing a real boundary edge each draw their own wall
    there -- a real, honest double-wall simplification for a first slice,
    not a hidden approximation: deduplicating shared edges would need a
    real adjacency reconciliation (`InteriorCell.connects_to` says WHICH
    cells are adjacent, not which specific edge they share), a separate,
    larger lift not attempted here.
    """
    solids = []
    n = len(cell_ring_xy)
    for i in range(n):
        ax, ay = cell_ring_xy[i]
        bx, by = cell_ring_xy[(i + 1) % n]
        dx, dy = bx - ax, by - ay
        length = math.hypot(dx, dy)
        if length < 1e-6:
            continue
        nx, ny = -dy / length, dx / length  # unit perpendicular to the edge
        hw = wall_thickness_m / 2
        quad = [
            (ax + nx * hw, ay + ny * hw), (bx + nx * hw, by + ny * hw),
            (bx - nx * hw, by - ny * hw), (ax - nx * hw, ay - ny * hw),
        ]
        try:
            solid = cq.Workplane("XY").polyline(quad).close().extrude(wall_height_m)
            if z_offset_m:
                solid = solid.translate((0, 0, z_offset_m))
            solids.append(solid)
        except Exception as e:
            print(f"  ! skipped an interior wall segment (extrude failed: {e})", file=sys.stderr)
    return solids


def footprint_slab_solid(outer_xy, holes_xy, thickness_m, z_offset_m=0.0):
    """A thin horizontal slab across one real footprint (already local XY
    meters) -- the same multi-wire-on-one-workplane hole technique
    `extrude_polygon` uses, just taking XY directly since every caller
    here already has it computed. Used for `include_interior_walls`'s real
    per-floor floor plates -- see `build_scene`'s own docstring for why
    those exist. Reuses the building's own real OUTER ring for every
    floor's plate -- this schema has no per-floor footprint field either
    (the same real gap `roof_cap_solid`'s own docstring notes at the
    roof), so a real footprint that doesn't change floor to floor is the
    honest, not fabricated, choice."""
    if len(outer_xy) < 3:
        return None
    wp = cq.Workplane("XY").polyline(outer_xy).close()
    for hole_xy in holes_xy:
        if len(hole_xy) >= 3:
            wp = wp.polyline(hole_xy).close()
    try:
        solid = wp.extrude(thickness_m)
        if z_offset_m:
            solid = solid.translate((0, 0, z_offset_m))
        return solid
    except Exception as e:
        print(f"  ! skipped a floor plate (extrude failed: {e})", file=sys.stderr)
        return None


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


def find_pocket_refill(building_outer_xy, pocket_outer_xy):
    """Join a P124 Activity Pockets `open_space` entry (`kind: "pocket"`)
    back to the real building it bumps out from -- purely from geometry
    this scene already has, no Rust schema change needed.

    `p124_activity_pockets` (crates/street-smarts-patterns) splices the
    pocket's own 4 corners directly into its parent building's outer ring
    at the bump site, and the SAME 4 points (run through the SAME
    local<->lnglat conversion, same origin) become the Pocket's own
    `open_space` polygon -- so at least 3 of the pocket's 4 vertices are,
    bit-for-bit, real vertices of the FINAL (post-bump) building ring this
    scene already has. Requiring 3+ (not just 1) is what makes this safe
    against a party-wall neighbor (P108-merged buildings can share one or
    two incidental ring vertices, never a run of 3+) -- confirmed on the
    real eastside-baseline fixture back when the generator produced real
    pockets (17/17 matched exactly one real building each, zero ambiguous
    matches); the matching logic itself is unchanged by the later
    notch-to-bump rewrite, since the splice technique is the same either
    direction.

    Returns the set of `ring_index` values (edge-start indices into
    `building_outer_xy`) for the short bump edges (the nook's own
    side/front walls) -- used both by `build_scene`'s own cutback step and
    to tell `opening_records` which edges don't exist above ground floor
    once the bump is cut back out. Empty set if `pocket_outer_xy` isn't
    this building's own pocket.
    """
    idxs = []
    for pp in pocket_outer_xy:
        for i, bp in enumerate(building_outer_xy):
            if math.hypot(pp[0] - bp[0], pp[1] - bp[1]) < POCKET_MATCH_EPS_M:
                idxs.append(i)
                break
    if len(idxs) < 3:
        return set()
    idxs_sorted = sorted(idxs)
    n = len(building_outer_xy)
    return {
        idxs_sorted[i]
        for i in range(len(idxs_sorted) - 1)
        if (idxs_sorted[i + 1] - idxs_sorted[i]) % n == 1
    }


def _point_to_segment_dist(px, py, ax, ay, bx, by):
    """Real Euclidean distance from point `(px, py)` to segment `(a, b)`
    (already local meters) -- standard clamped-projection formula, used by
    `_min_dist_point_to_ring` to find a real InteriorCell's own exterior-
    facing edge without requiring exact vertex coincidence."""
    dx, dy = bx - ax, by - ay
    length2 = dx * dx + dy * dy
    if length2 < 1e-12:
        return math.hypot(px - ax, py - ay)
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / length2))
    cx, cy = ax + t * dx, ay + t * dy
    return math.hypot(px - cx, py - cy)


def _min_dist_point_to_ring(px, py, ring_xy):
    """Real minimum distance from `(px, py)` to any edge of `ring_xy`
    (already local meters, closed implicitly like every other ring in
    this file)."""
    n = len(ring_xy)
    return min(
        _point_to_segment_dist(px, py, ring_xy[i][0], ring_xy[i][1], ring_xy[(i + 1) % n][0], ring_xy[(i + 1) % n][1])
        for i in range(n)
    )


def _ray_exit_distance(cx, cy, dirx, diry, ring_xy, max_dist=60.0):
    """Real distance from `(cx, cy)` to where a ray in direction `(dirx,
    diry)` (unit vector) first exits the closed polygon `ring_xy` --
    standard ray/segment intersection against every real edge, smallest
    positive `t` wins. `max_dist` if the ray never exits within that
    range (degenerate ring). Used by `_best_interior_view_direction` to
    find how much real open room space lies in a given direction."""
    best = max_dist
    n = len(ring_xy)
    for i in range(n):
        ax, ay = ring_xy[i]
        bx, by = ring_xy[(i + 1) % n]
        ex, ey = bx - ax, by - ay
        denom = dirx * ey - diry * ex
        if abs(denom) < 1e-9:
            continue
        t = ((ax - cx) * ey - (ay - cy) * ex) / denom
        s = ((ax - cx) * diry - (ay - cy) * dirx) / denom
        if t > 1e-6 and 0.0 <= s <= 1.0:
            best = min(best, t)
    return best


def _best_interior_view_direction(cx, cy, cell_xy, outward_xy, n_samples=24):
    """The real direction from a room's own centroid `(cx, cy)` that gives
    an "inside looking out" camera the most legible composition -- swept
    over `n_samples` directions around the compass, scored by how much
    real open room space (`_ray_exit_distance`, against the room's OWN
    real polygon) lies that way, biased toward directions that also point
    somewhat toward the room's own real exterior wall (`outward_xy`, from
    `interior_view_candidates`'s own edge-normal calculation).

    Exists because the raw wall normal alone produced a bad shot on the
    real fixture: an InteriorCell's own real depth band can be shallow
    (confirmed: as little as ~5m) relative to how far a camera needs to
    pull back to read as "inside a room" rather than "pressed against a
    wall" -- facing that normal directly put the camera behind or inside
    the real exterior wall solid. The room's own real LONG axis (an
    elongated p127_intimacy_gradient band, common along a building's
    public-facing side) has real open depth a camera can actually sit
    inside; empirically confirmed (real screenshots, not assumed) to read
    as a legible interior corridor/room shot where the raw wall-normal
    direction read as a flat close-up of a single wall face. The
    `outward_xy` bias keeps the choice from swinging a full 180 degrees
    into a direction that faces away from any real window entirely.

    Returns `(direction_xy, ray_distance_m)` -- the real open-space depth
    in the chosen direction, so a caller can size a camera radius that
    stays inside that real room instead of pulling back into whatever's
    beyond it.
    """
    best_dir, best_dist, best_score = outward_xy, 0.0, -1.0
    for i in range(n_samples):
        angle = 2 * math.pi * i / n_samples
        dirx, diry = math.cos(angle), math.sin(angle)
        dist = _ray_exit_distance(cx, cy, dirx, diry, cell_xy)
        alignment = dirx * outward_xy[0] + diry * outward_xy[1]
        score = dist * (0.5 + 0.5 * max(0.0, alignment))
        if score > best_score:
            best_score = score
            best_dir = (dirx, diry)
            best_dist = dist
    return best_dir, best_dist


def polygon_signed_area2_m2(ring_xy):
    """Signed shoelace sum (twice the real area) of a ring already in
    local meters -- positive iff `ring_xy` winds counter-clockwise. Used
    only to derive a ring's own real outward-normal direction generically
    (`opening_placement`'s own docstring) -- this file doesn't itself
    enforce one winding convention for a hole ring vs its building's own
    outer ring, so deriving it per-ring from the real data beats assuming
    one."""
    n = len(ring_xy)
    if n < 3:
        return 0.0
    total = 0.0
    for i in range(n):
        x1, y1 = ring_xy[i]
        x2, y2 = ring_xy[(i + 1) % n]
        total += x1 * y2 - x2 * y1
    return total


def opening_placement(ring_xy, ring_sign, o):
    """Real placement for one `Opening` -- edge point (`ring_index` + `t`),
    vertical position (`floor * FLOOR_TO_FLOOR_M + sill_height_m`), and
    real width/height. This used to feed a real OpenCascade boolean punch
    (a deep box, cut out of the wall solid); now it feeds a flat decal
    instead (see this file's own module doc for the real measured
    reasons) -- kept as ONE function either way, since `render_isometric`'s
    flat quad and `export_glb`'s thin decal box both need the exact same
    real numbers, just turned into different final geometry.

    Each `Opening` references a wall edge by `ring_index`/`on_hole` into
    `building["polygon"]["outer"]` or `["holes"][0]` (the SAME rings the
    Rust operator indexed, using its own per-building local projection --
    `ring_index` is just an array index and `t` a fraction of that edge's
    own length, both invariant to which nearby origin the lng/lat->meters
    projection used, so reusing this scene's shared origin here is exact,
    not approximate).

    Returns `(mx, my, z_center, angle_deg, width, height, nx, ny)` --
    `(nx, ny)` is the ring's own real OUTWARD unit normal at this edge,
    derived from `ring_sign` (`polygon_signed_area2_m2`: positive means
    CCW) so a decal offsets away from solid mass regardless of which way
    this particular ring happens to wind. `None` if the referenced edge is
    degenerate or out of range (real, not hypothetical -- the same cases
    the old boolean punch had to skip).
    """
    n = len(ring_xy)
    i = o["ring_index"]
    if n < 2 or i >= n:
        return None
    ax, ay = ring_xy[i]
    bx, by = ring_xy[(i + 1) % n]
    dx, dy = bx - ax, by - ay
    edge_len = math.hypot(dx, dy)
    if edge_len < 1e-6:
        return None
    t = o["t"]
    mx, my = ax + dx * t, ay + dy * t
    # Outward normal: for a CCW ring, the -90 deg rotation of the edge
    # direction points away from the ring's own interior (verified against
    # a CCW unit square's own bottom edge: direction (1,0) -> outward
    # (0,-1), i.e. downward, away from the square) -- +90 deg for a CW
    # ring instead, since a hole ring commonly (but not, in this file,
    # guaranteed to) wind opposite its building's own outer ring.
    nx, ny = (dy / edge_len, -dx / edge_len) if ring_sign >= 0 else (-dy / edge_len, dx / edge_len)
    angle_deg = math.degrees(math.atan2(dy, dx))
    width = max(o["width_m"], 0.1)
    height = max(o["head_height_m"] - o["sill_height_m"], 0.1)
    z_bottom = o["floor"] * FLOOR_TO_FLOOR_M + o["sill_height_m"]
    return mx, my, z_bottom + height / 2, angle_deg, width, height, nx, ny


def opening_quad_corners(placement):
    """`render_isometric`'s own use of `opening_placement`: the 4 real
    corners (3D) of a flat rectangle sitting `OPENING_DECAL_OFFSET_M`
    proud of the wall, in wall-plane winding order (matplotlib's
    `Poly3DCollection` draws an n-gon face directly, no triangle split
    needed for its own sake)."""
    mx, my, z_center, angle_deg, width, height, nx, ny = placement
    ang = math.radians(angle_deg)
    ux, uy = math.cos(ang), math.sin(ang)
    ox, oy = mx + nx * OPENING_DECAL_OFFSET_M, my + ny * OPENING_DECAL_OFFSET_M
    half_w, half_h = width / 2, height / 2
    z_lo, z_hi = z_center - half_h, z_center + half_h
    return [
        (ox - ux * half_w, oy - uy * half_w, z_lo),
        (ox + ux * half_w, oy + uy * half_w, z_lo),
        (ox + ux * half_w, oy + uy * half_w, z_hi),
        (ox - ux * half_w, oy - uy * half_w, z_hi),
    ]


def opening_records(building, origin_lng, origin_lat, skip_ring_indices=frozenset()):
    """Every real placement + color for one building's `openings` --
    `build_scene`'s own single pass over the real `Opening` data, computed
    once and shared by both `render_isometric` (turns each into a flat
    quad) and `export_glb` (turns each into a thin decal solid), instead
    of each re-deriving position/orientation from scratch.

    `skip_ring_indices` (outer-ring edges only, never holes): P124
    Activity Pockets runs BEFORE P221 in the real pipeline, so P221 places
    openings against the already-bumped ring and can put a floor>=1
    opening on one of the bump's own short edges. `build_scene`'s own
    pocket-cutback step cuts that bump back out above ground level (see
    `find_pocket_refill`'s own docstring) -- for a building it cut back,
    those specific edges DON'T EXIST at floor>=1 after the cutback (the
    ring above ground floor reverts to the plain, pre-bump footprint), so
    a decal there would float in open air or sit glued inside solid mass
    depending on direction, either way not a real opening. Floor-0
    openings on the same edges are untouched -- ground level really does
    keep the bump. Not yet re-measured against a real fixture with real
    bumped pockets (the current eastside-baseline fixture produces zero
    real pockets under the corrected, Alexander-faithful generator -- see
    p124_activity_pockets.rs's own module doc) -- kept as a cheap,
    always-correct guard rather than something to defer until a fixture
    with real pockets exists to check it against.
    """
    openings = building.get("openings") or []
    if not openings:
        return []

    outer_ring = ring_to_xy(building["polygon"]["outer"], origin_lng, origin_lat)
    outer_sign = polygon_signed_area2_m2(outer_ring)
    holes = building["polygon"].get("holes") or []
    hole_ring = ring_to_xy(holes[0], origin_lng, origin_lat) if holes else None
    hole_sign = polygon_signed_area2_m2(hole_ring) if hole_ring else 0.0

    records = []
    for o in openings:
        on_hole = o.get("on_hole")
        if not on_hole and o.get("floor", 0) >= 1 and o["ring_index"] in skip_ring_indices:
            continue
        ring = hole_ring if on_hole else outer_ring
        ring_sign = hole_sign if on_hole else outer_sign
        if not ring:
            continue
        placement = opening_placement(ring, ring_sign, o)
        if placement is None:
            continue
        is_door = o["kind"] == "door"
        color_key = "opening_door" if is_door else ("opening_window_courtyard" if on_hole else "opening_window")
        records.append((placement, color_key))
    return records


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


def build_scene(nbhd, context_path=None, include_interior_walls=False):
    """`include_interior_walls`: real, opt-in -- OFF by default so every
    existing scenario (clean_baseline included, since p127_intimacy_
    gradient runs on every real building site-wide, not just a showcase
    cluster) renders exactly as it always has, byte-for-byte, unless a
    caller explicitly asks for the interior-detail treatment (see
    `interior_partition_solids`'s own docstring, and `main`'s own
    `--interiors` flag). When on, `render_isometric`/`export_glb` both
    key off whether `scene["interior_walls"]` is non-empty to lower the
    exterior shell's own alpha and draw partitions on top -- no separate
    render-time flag needed there, the scene data itself is the signal.
    """
    parcels = site_parcels(nbhd)
    if not parcels:
        raise SystemExit("no site parcels found -- check spec filtering")

    # Origin = centroid of all site parcel vertices.
    all_pts = [p for parcel in parcels for p in parcel["polygon"]["outer"]]
    origin_lng = sum(p["lng"] for p in all_pts) / len(all_pts)
    origin_lat = sum(p["lat"] for p in all_pts) / len(all_pts)

    building_solids = []  # (solid, color_name) -- unpunched: window/door
    # openings are a separate flat-decal layer now (`opening_decal_records`
    # below), not a boolean cut into this solid -- see this file's own
    # module doc for the real measured reason. Both `render_isometric` and
    # `export_glb` share this SAME list now; there is no more separate
    # punched-vs-unpunched pair to keep in sync.
    opening_decal_records = []  # [(placement, color_key), ...] -- see opening_records's own docstring
    interior_wall_solids = []  # [(solid, "interior_wall"), ...] -- real
    # p127_intimacy_gradient InteriorCell partitions, only ever populated
    # when include_interior_walls=True.
    building_ids_with_real_shape = set()

    # P124 Activity Pockets' own `open_space` entries (kind="pocket") --
    # matched to their parent building below, per pocket, via
    # find_pocket_refill's own vertex join. A pocket is claimed (popped)
    # once matched: P124's own real constraint is at most one pocket per
    # building, confirmed on the real fixture (17/17 unique matches), so
    # this also means a building can never be refilled twice.
    unclaimed_pockets = [o for o in nbhd.get("open_space", []) if o.get("kind") == "pocket"]

    # Real P107-shaped buildings first (real height, may have a courtyard hole).
    # `polygon.get("parts")` is always a single element for buildings this
    # pipeline emits (P107 never produces a multi-part Building) -- opening
    # ring_index/on_hole reference the building's own top-level outer/holes,
    # so opening_records assumes that single-part case; not handled in
    # general for a hypothetical multi-part building.
    # Throwaway coarse profiling (not wired into any test, just this
    # module's own stderr) -- attributes build_scene's own wall-clock time
    # to its real phases, since a bare end-to-end script timing can't
    # (confirmed: +/-50s sandbox noise swamped a single before/after
    # comparison when the P124 refill was added). Kept cheap: time.perf_counter()
    # calls around work already happening, no extra passes.
    t_extrude = t_refill = t_openings = t_roof = t_interior = 0.0

    for b in nbhd.get("buildings", []):
        height = b.get("height_m") or DEFAULT_BUILDING_HEIGHT_M
        parts = b["polygon"].get("parts") or [{"outer": b["polygon"]["outer"], "holes": b["polygon"].get("holes", [])}]
        for part in parts:
            _t0 = time.perf_counter()
            solid = extrude_polygon(part["outer"], part.get("holes", []), height, origin_lng, origin_lat)
            t_extrude += time.perf_counter() - _t0
            if solid is None:
                continue

            # Volumetric cutback: a P124 pocket is a GROUND-LEVEL alcove (see
            # p124_activity_pockets.rs's own module doc -- Alexander's "small
            # pocket of activity" reading is a street-level feature, not a
            # floor-to-roof bay window), but `extrude_polygon` above just
            # swept the (already-bumped) ring straight up by the building's
            # FULL height -- a naive single extrusion projects the bump
            # through every floor, not just the ground one. There's no
            # per-floor footprint field anywhere in this schema to read the
            # "right" upper-floor shape from, and no pre-bump ring either
            # (P124 deliberately doesn't keep one -- every downstream Rust
            # consumer needs the FINAL ring). Re-derive it instead from the
            # pocket's own emitted geometry: extrude just the pocket
            # rectangle from one floor-to-floor height up to the roof, and
            # CUT it back out of the base solid, so only the ground floor
            # keeps the projecting bump and every floor above reverts to the
            # plain, pre-bump facade -- a ground-floor nook, not a bay
            # window repeated at every story. This is the opposite boolean
            # from the earlier inward-notch reading (which added material
            # back above ground floor); the base ring itself now already
            # includes the bump, so the correction needed above ground
            # floor is subtractive.
            _t0 = time.perf_counter()
            bump_edge_indices = set()
            outer_xy = ring_to_xy(part["outer"], origin_lng, origin_lat)
            for pocket in unclaimed_pockets:
                pocket_xy = ring_to_xy(pocket["polygon"]["outer"], origin_lng, origin_lat)
                edges = find_pocket_refill(outer_xy, pocket_xy)
                if not edges:
                    continue
                bump_edge_indices = edges
                if height > FLOOR_TO_FLOOR_M:
                    # Assumes the pocket itself is exactly one
                    # FLOOR_TO_FLOOR_M tall -- P124 carries no explicit
                    # height field of its own, so this is a rendering-layer
                    # assumption, not a Rust-derived fact.
                    cutback = extrude_polygon(
                        pocket["polygon"]["outer"], [], height - FLOOR_TO_FLOOR_M, origin_lng, origin_lat
                    )
                    if cutback is not None:
                        cutback = cutback.translate((0, 0, FLOOR_TO_FLOOR_M))
                        try:
                            solid = solid.cut(cutback).clean()
                        except Exception as e:
                            print(
                                f"  ! pocket cutback failed for {b.get('id')}, "
                                f"rendering with the full-height bump instead: {e}",
                                file=sys.stderr,
                            )
                unclaimed_pockets.remove(pocket)
                break
            t_refill += time.perf_counter() - _t0

            building_solids.append((solid, "building_shaped"))

            # P117 Sheltering Roof's own real RoofForm (crates/street-smarts-
            # core/src/nir.rs) -- a real triangular-wedge shed-roof cap sitting
            # ABOVE the wall extrusion above (eave_height_m == this building's
            # own real height_m, so the cap starts exactly where the walls
            # already end), added as an independent solid rather than
            # unioned into `solid` -- see roof_cap_solid's own docstring for
            # why a plain extrusion, not a boolean, is the right real cost
            # here.
            _t0 = time.perf_counter()
            roof = b.get("roof")
            if roof is not None:
                roof_solid = roof_cap_solid(
                    part["outer"], roof["eave_height_m"], roof["ridge_height_m"], origin_lng, origin_lat
                )
                if roof_solid is not None:
                    building_solids.append((roof_solid, "roof"))
            t_roof += time.perf_counter() - _t0

            _t0 = time.perf_counter()
            opening_decal_records.extend(
                opening_records(b, origin_lng, origin_lat, skip_ring_indices=bump_edge_indices)
            )
            t_openings += time.perf_counter() - _t0

            # p127_intimacy_gradient's own real InteriorCell partitions --
            # only ever built when a caller explicitly opts in (see
            # build_scene's own docstring); every existing scenario leaves
            # `nbhd["buildings"][*]["interior_cells"]` untouched here.
            #
            # `InteriorCell.floor` is hard-coded 0 in this schema -- there's
            # only ever ONE real computed room layout per building, not a
            # different one per story. Rather than draw that single layout
            # floating at one height inside an otherwise-empty floor-to-roof
            # volume (what this used to do, and what made the whole cluster
            # read as a "pie tin" -- thin partitions with nothing above or
            # below them), the SAME real layout is repeated at every real
            # floor level (`b["floors"]`, falling back to `height /
            # FLOOR_TO_FLOOR_M` when that field is absent) -- an honestly-
            # labeled real approximation (see interior_partition_solids' own
            # docstring), not a claim that a different plan exists up there.
            # A thin real floor plate at each level (skipping floor 0, which
            # already sits on the real ground) gives the stack an actual
            # floor-to-floor read instead of walls alone floating in air.
            if include_interior_walls:
                cells = b.get("interior_cells") or []
                if cells:
                    _t0 = time.perf_counter()
                    wall_thickness = interior_wall_thickness_for(b)
                    real_floor_count = b.get("floors") or max(1, round(height / FLOOR_TO_FLOOR_M))
                    holes_xy = [ring_to_xy(h, origin_lng, origin_lat) for h in part.get("holes", [])]
                    for floor_idx in range(real_floor_count):
                        z_offset = floor_idx * FLOOR_TO_FLOOR_M
                        remaining = height - z_offset
                        if remaining <= 0.05:
                            break
                        # Capped at the building's own real remaining height
                        # so a top floor's walls don't poke up through the
                        # roof cap above them -- a real geometric
                        # constraint, matching roof_cap_solid's own eave/
                        # ridge relationship, not a guess.
                        floor_wall_height = min(INTERIOR_WALL_HEIGHT_M, remaining - FLOOR_PLATE_THICKNESS_M)
                        if floor_wall_height <= 0:
                            continue
                        for cell in cells:
                            cell_xy = ring_to_xy(cell["polygon"]["outer"], origin_lng, origin_lat)
                            if len(cell_xy) < 3:
                                continue
                            for wall_solid in interior_partition_solids(
                                cell_xy, wall_height_m=floor_wall_height,
                                wall_thickness_m=wall_thickness, z_offset_m=z_offset,
                            ):
                                interior_wall_solids.append((wall_solid, "interior_wall"))
                        if floor_idx >= 1:
                            slab = footprint_slab_solid(outer_xy, holes_xy, FLOOR_PLATE_THICKNESS_M, z_offset_m=z_offset)
                            if slab is not None:
                                interior_wall_solids.append((slab, "floor_plate"))
                    t_interior += time.perf_counter() - _t0
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

    # Plazas / open space -- thin colored slabs at ground level. A P124
    # pocket gets one of these too (its own real footprint, distinctly
    # colored -- see COLORS) IN ADDITION TO the volumetric refill above:
    # the ground-level slab reads as the pocket's own activity surface, the
    # refill is what makes the adjacent building's upper floors read as
    # intact mass instead of a floor-to-roof slot.
    plaza_solids = []
    for o in nbhd.get("open_space", []):
        parts = o["polygon"].get("parts") or [{"outer": o["polygon"]["outer"], "holes": o["polygon"].get("holes", [])}]
        for part in parts:
            solid = extrude_polygon(part["outer"], part.get("holes", []), PLAZA_THICKNESS_M, origin_lng, origin_lat)
            if solid is not None:
                kind = o.get("kind") if o.get("kind") in ("undecided", "common", "pocket") else "plaza"
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

    # Activity nodes -- real point markers (P61 Small Public Squares, P124
    # Activity Pockets both produce these) that this renderer never drew at
    # all before now, not even as a missing sub-field: `Neighborhood.
    # activity_nodes` was never referenced anywhere in this file. Cheap
    # (21 real nodes on the clean_baseline fixture, vs. the 7,765 openings
    # that forced a real budget decision) -- a small square post, real
    # height, real footprint, colored by the node's own real `kind`
    # (ActivityKind, confirmed against the Rust enum -- see COLORS' own
    # comment). No design ambiguity worth agonizing over here the way
    # openings/interiors had real cost tradeoffs to weigh.
    activity_solids = []
    for a in nbhd.get("activity_nodes", []):
        loc = a.get("location")
        if not loc:
            continue
        x, y = project(loc["lng"], loc["lat"], origin_lng, origin_lat)
        r = ACTIVITY_MARKER_RADIUS_M
        square = [(x - r, y - r), (x + r, y - r), (x + r, y + r), (x - r, y + r)]
        try:
            solid = cq.Workplane("XY").polyline(square).close().extrude(ACTIVITY_MARKER_HEIGHT_M)
            kind = a.get("kind") or "other"
            activity_solids.append((solid, f"activity_{kind}"))
        except Exception as e:
            print(f"  ! activity marker build failed: {e}", file=sys.stderr)

    _t0 = time.perf_counter()
    context_solids = load_context_buildings(context_path, origin_lng, origin_lat) if context_path else []
    t_context = time.perf_counter() - _t0

    print(
        f"  build_scene timing: extrude={t_extrude:.2f}s refill={t_refill:.2f}s roof={t_roof:.2f}s "
        f"opening_records={t_openings:.2f}s interior={t_interior:.2f}s context={t_context:.2f}s"
    )

    return {
        "buildings": building_solids,
        "opening_decals": opening_decal_records,
        "interior_walls": interior_wall_solids,
        "plazas": plaza_solids,
        "streets": street_solids,
        "activity_markers": activity_solids,
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
    "roof": "#5c3a2e",  # a real, darker shingled-roof brown -- distinct from
    # "building_shaped" but clearly related (same warm-brown family), not an
    # unrelated hue, matching "pocket"'s own reasoning above.
    "interior_wall": "#e8dfc8",  # a real, light plaster-like tone -- deliberately
    # far from the warm-brown exterior family so a real InteriorCell partition
    # reads as a distinct, interior element seen through the translucent shell,
    # not another shade of the same building mass.
    "floor_plate": "#9c9482",  # a real, gray-tan concrete-slab tone -- close
    # enough to "interior_wall" to read as part of the same real stacked-
    # interior system, distinct enough (cooler, grayer) not to be confused
    # with a vertical partition when seen edge-on between floors.
    "plaza": "#d9a441",
    "pocket": "#c9713f",  # warm, between "building_shaped" and "plaza" --
    # reads as related to both (it's carved from the one, opens onto the
    # other) rather than a third unrelated hue.
    "common": "#a3b18a",
    "undecided": "#b8602a",
    "local": "#6b6259",
    "pedestrian": "#9b8f7a",
    "street": "#6b6259",
    "context": "#5a5a5e",  # flat neutral gray -- deliberately duller than every
    # generated-site color so real Overture context reads as backdrop, not
    # competes with the pattern-language buildings it's surrounding.
    "opening_window": WINDOW_COLOR,
    "opening_window_courtyard": COURTYARD_WINDOW_COLOR,
    "opening_door": DOOR_COLOR,
    # ActivityNode markers -- one real color per ActivityKind variant
    # (street-smarts-core::nir::ActivityKind, `#[serde(rename_all =
    # "snake_case")]`, confirmed directly against the Rust enum, not
    # guessed) -- deliberately a saturated, un-earthy palette distinct
    # from the site's own warm massing colors, since these are meant to
    # read as point markers, not blend into the buildings/ground.
    "activity_commerce": "#c2542f",
    "activity_civic": "#2e6b4f",
    "activity_transit": "#4f7d96",
    "activity_school": "#b8933a",
    "activity_worship": "#7a5ea8",
    "activity_health": "#c23f5a",
    "activity_other": "#8a8a8a",
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


def draw_solid_group(ax, items, alpha, bounds_accum=None):
    """Tessellate and draw one real `(solid, kind)` list onto `ax` --
    shared by `render_isometric` and `render_interior_view` so both draw
    real geometry through the exact same shading/coloring path, not two
    copies that could quietly drift apart. `bounds_accum`, if given, is
    the SAME camera-framing accumulation list `render_isometric` already
    used (an `append`-only list of vertex arrays) -- `render_interior_view`
    passes `None` since it frames its own fixed, tight bounds instead of
    fitting the whole scene.
    """
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
        if bounds_accum is not None:
            bounds_accum.append(verts)


def draw_opening_decals(ax, records, alpha=0.95, bounds_accum=None):
    """Flat, unshaded quads (no cadquery/OCC involved at all -- see this
    file's own module doc for why a real boolean punch was replaced with
    this) -- grouped by `color_key` since `shade_faces`/a single
    `Poly3DCollection` call needs one color per call, and window/
    courtyard-window/door each carry a real, distinct color (matching
    `render_largest_building_floors`'s own 2D convention). Shared by
    `render_isometric` and `render_interior_view` -- see `draw_solid_
    group`'s own docstring for why."""
    if not records:
        return
    by_color = {}
    for placement, color_key in records:
        by_color.setdefault(color_key, []).append(opening_quad_corners(placement))
    for color_key, quads in by_color.items():
        face_verts = np.array(quads)  # (n, 4, 3)
        base_rgb = np.array(mcolors.to_rgb(COLORS.get(color_key, "#999999")))
        rgba = np.tile(np.append(base_rgb, alpha), (len(quads), 1))
        poly = Poly3DCollection(face_verts, facecolors=rgba, edgecolor="none")
        ax.add_collection3d(poly)
        if bounds_accum is not None:
            bounds_accum.append(face_verts.reshape(-1, 3))


def render_isometric(scene, out_path, title):
    fig = plt.figure(figsize=(10, 10))
    ax = fig.add_subplot(111, projection="3d")
    fig.patch.set_facecolor("#2a2a2e")
    ax.set_facecolor("#2a2a2e")

    # Camera-framing bounds accumulate here as each group is actually
    # drawn, instead of a SECOND full pass re-tessellating every
    # street/plaza/building solid again afterward just to find min/max --
    # that used to mean every real OpenCascade tessellation in this
    # function ran twice. `context` is deliberately excluded (see its own
    # comment below, unchanged) -- only calls that pass a real
    # `bounds_accum` list contribute.
    bounds_verts = []

    def add_group(items, alpha, for_bounds=False):
        draw_solid_group(ax, items, alpha, bounds_accum=bounds_verts if for_bounds else None)

    def add_opening_decals(records, alpha=0.95, for_bounds=True):
        draw_opening_decals(ax, records, alpha, bounds_accum=bounds_verts if for_bounds else None)

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
    add_group(scene["streets"], alpha=0.55, for_bounds=True)
    add_group(scene["plazas"], alpha=0.6, for_bounds=True)

    # A real `interior_walls` layer (only ever non-empty when a caller
    # opted into `include_interior_walls`, see build_scene's own
    # docstring) changes what the exterior shell is FOR: normally it's the
    # subject, drawn near-opaque; with real partitions to show, it becomes
    # a translucent shell so the partitions read as visible interior
    # structure rather than being hidden inside solid mass. Every existing
    # scenario leaves `interior_walls` empty, so `buildings_alpha` stays
    # 0.82 and this whole render is byte-for-byte what it always was.
    interior_walls = scene.get("interior_walls", [])
    buildings_alpha = 0.35 if interior_walls else 0.82
    add_group(scene["buildings"], alpha=buildings_alpha, for_bounds=True)
    add_opening_decals(scene.get("opening_decals", []))
    add_group(interior_walls, alpha=0.95, for_bounds=True)
    add_group(scene.get("activity_markers", []), alpha=0.95, for_bounds=True)

    all_verts = np.concatenate(bounds_verts, axis=0)
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


INTERIOR_VIEW_RADIUS_M = 9.0  # how far the tight interior-view camera
# frames in every direction from its own aim point -- big enough to show
# the room the camera sits in AND real geometry past the exterior wall
# (plaza/street/neighboring building), small enough to stay a real close-
# up, not another wide shot of the whole cluster.


def interior_view_candidates(nbhd, origin_lng, origin_lat, max_views=2):
    """Pick up to `max_views` real (building, cell, floor_idx) rooms to
    frame an "inside looking out" shot around -- real p127_intimacy_
    gradient InteriorCell data, not staged positions. Prefers each real
    building's own "entrance" cell (`kind == "entrance"`, the one this
    schema itself already treats as the public-facing room -- see
    p110_main_entrance/p127's own docs) since that's the cell most likely
    to sit against a real exterior wall with real p221 window/door
    openings on it; falls back to the minimum-depth cell when a building
    has no cell tagged "entrance". Always floor 0 -- the one real floor
    p127 itself computed a layout for (see interior_partition_solids' own
    docstring for why every OTHER floor in the render is the same real
    layout, just repeated).

    Ranks candidate buildings by real height (tallest first) -- the
    tallest real building in a cluster is also the one `p117_sheltering_
    roof` gives the most real interior volume above these rooms, so its
    own shot has the most real context. Returns a list of dicts with keys
    `building`, `cell`, `floor_idx`, `room_xy` (real cell centroid, local
    meters), `outward_xy` (real unit direction to aim the camera along --
    `_best_interior_view_direction`'s own pick, not simply the wall's own
    outward normal; see its docstring for why) -- empty if no building in
    `nbhd` carries any real `interior_cells`.
    """
    candidates = []
    ranked = sorted(
        (b for b in nbhd.get("buildings", []) if b.get("interior_cells")),
        key=lambda b: b.get("height_m") or 0.0,
        reverse=True,
    )
    for b in ranked[:max_views]:
        cells = b["interior_cells"]
        cell = next((c for c in cells if c.get("kind") == "entrance"), None)
        if cell is None:
            cell = min(cells, key=lambda c: c.get("depth", 1.0))
        outer_xy = ring_to_xy(b["polygon"]["outer"], origin_lng, origin_lat)
        cell_xy = ring_to_xy(cell["polygon"]["outer"], origin_lng, origin_lat)
        if len(outer_xy) < 3 or len(cell_xy) < 3:
            continue
        ccx = sum(p[0] for p in cell_xy) / len(cell_xy)
        ccy = sum(p[1] for p in cell_xy) / len(cell_xy)
        # The room's own real exterior-facing edge -- a depth-0 InteriorCell
        # is built from a strip against the building's own outer ring (see
        # p127_intimacy_gradient's own module doc), so one of its own edges
        # runs along (or very near) the ring. Found by real point-to-segment
        # distance from each of the cell's own edge midpoints to the ring
        # (`_min_dist_point_to_ring`), not by requiring exact vertex
        # coincidence (`shared_boundary`'s own technique, tried first here
        # and confirmed to miss real cases: any building whose depth-0 band
        # is built as an inset offset rather than a literal ring-vertex
        # reuse has NO exactly-coincident edge, even though it obviously
        # still has a real wall side). Its outward normal is the real wall
        # direction this shot should face -- far more reliable than a
        # building-centroid-to-room-centroid guess, which pointed the wrong
        # way whenever a room's own real shape wasn't centered on its
        # building (confirmed visually: the centroid heuristic framed a
        # shot looking almost edge-on down a long real facade instead of
        # through it).
        n = len(cell_xy)
        best = None
        for i in range(n):
            ax, ay = cell_xy[i]
            bx, by = cell_xy[(i + 1) % n]
            # Skip degenerate (near-zero-length) edges outright -- a real
            # ring can carry a repeated vertex (confirmed on the real
            # fixture: one InteriorCell ring had an exact duplicate point),
            # and a zero-length "edge" trivially has distance 0 to itself,
            # which would otherwise always win the comparison below without
            # being a real wall side at all.
            if math.hypot(bx - ax, by - ay) < 1e-6:
                continue
            mx, my = (ax + bx) / 2, (ay + by) / 2
            d = _min_dist_point_to_ring(mx, my, outer_xy)
            if best is None or d < best[0]:
                best = (d, ax, ay, bx, by)
        if best is None:
            continue
        _, ex1, ey1, ex2, ey2 = best
        edx, edy = ex2 - ex1, ey2 - ey1
        edge_len = math.hypot(edx, edy)
        nx, ny = edy / edge_len, -edx / edge_len  # perpendicular to the edge
        if (ccx - (ex1 + ex2) / 2) * nx + (ccy - (ey1 + ey2) / 2) * ny > 0:
            nx, ny = -nx, -ny  # point AWAY from the room's own centroid
        view_dir, view_dist = _best_interior_view_direction(ccx, ccy, cell_xy, (nx, ny))
        # A camera radius sized to the real open space in the chosen
        # direction -- a fixed radius either sat inside a solid (a shallow
        # real room) or pulled back out of the room entirely (a deep real
        # one), confirmed on both real candidate rooms in this fixture.
        # Clamped to a sane real range either way.
        radius_m = max(1.5, min(8.0, view_dist * 0.55))
        candidates.append({
            "building": b, "cell": cell, "floor_idx": 0,
            "room_xy": (ccx, ccy), "outward_xy": view_dir, "radius_m": radius_m,
        })
    return candidates


INTERIOR_VIEW_EYE_HEIGHT_M = 1.6  # a plausible real eye height, matching
# INTERIOR_VIEW_RADIUS_M's own "plausible, honestly labeled" category.


def interior_camera_views(candidates):
    """Real `<model-viewer>` `camera-target`/`camera-orbit` strings for
    each `interior_view_candidates` entry -- lets a real hardware-
    accelerated WebGL camera be placed inside the SAME real `.glb` this
    file already publishes, instead of this file trying to fake a
    perspective render itself (`render_interior_view`'s own mplot3d
    camera has no true free eye position -- see its own docstring). The
    caller (the gallery page) still does the actual rendering; this is
    just real numbers, honestly derived from the same real room data.

    `export_glb`'s own root node carries a real -90-degree rotation about
    X (`cadquery`'s own Z-up-to-glTF-Y-up convention, confirmed by reading
    the exported `.glb`'s own node transform directly, not assumed) --
    `(x_local, y_local, z_local) -> (x_local, z_local, -y_local)` -- so
    every real local coordinate here is run through that same mapping
    before being handed to model-viewer, which places its camera in the
    GLB's own final (post-rotation) space.

    `theta`/`phi` follow model-viewer's own real spherical convention
    (confirmed directly against a live model-viewer instance via a
    headless-browser screenshot, not assumed from documentation alone):
    at `phi=90deg` the camera sits at `target + radius * (sin(theta), 0,
    cos(theta))`, so a camera meant to sit on the ROOM's own interior side
    of `direction_xy` (looking back along it) needs `theta = atan2(-dx,
    dy)`. `phi=82deg` (not a literal 90) tilts the camera very slightly
    down from dead-level -- an eye-level view reads as looking slightly
    into the floor/room rather than dead flat at the horizon, confirmed
    against the same live screenshots.
    """
    views = []
    for c in candidates:
        rx, ry = c["room_xy"]
        dirx, diry = c["outward_xy"]
        floor_z = c["floor_idx"] * FLOOR_TO_FLOOR_M
        target_gltf = (rx, floor_z + INTERIOR_VIEW_EYE_HEIGHT_M, -ry)
        theta_deg = math.degrees(math.atan2(-dirx, diry)) % 360
        views.append({
            "building_id": c["building"]["id"],
            "camera_target": f"{target_gltf[0]:.2f}m {target_gltf[1]:.2f}m {target_gltf[2]:.2f}m",
            "camera_orbit": f"{theta_deg:.1f}deg 82deg {c['radius_m']:.2f}m",
        })
    return views


def render_interior_view(scene, candidate, out_path, title):
    """A tight-cropped camera around ONE real interior room, aimed toward
    its own real exterior wall -- real geometry only (the same solids
    `render_isometric` draws), no synthetic room, no invented furniture or
    vantage point beyond where this real InteriorCell actually sits.

    Not a true first-person perspective render -- mplot3d's camera always
    orbits the CENTER of the current axis limits at a fixed elev/azim, it
    has no free eye position independent of that. The "inside looking
    out" effect here comes entirely from framing: axis limits cropped
    tightly around one real room (see `INTERIOR_VIEW_RADIUS_M`), a low
    `elev` (near eye level, not the wide-shot `elev=35` `render_isometric`
    uses), and `azim` aimed along the room's own real outward direction --
    an honest technique given the tool, not a claim this is raytraced.
    """
    fig = plt.figure(figsize=(10, 10))
    ax = fig.add_subplot(111, projection="3d")
    fig.patch.set_facecolor("#2a2a2e")
    ax.set_facecolor("#2a2a2e")

    draw_solid_group(ax, scene.get("interior_walls", []), alpha=0.95)
    draw_solid_group(ax, scene["buildings"], alpha=0.3)
    draw_opening_decals(ax, scene.get("opening_decals", []), alpha=0.95)
    draw_solid_group(ax, scene.get("plazas", []), alpha=0.5)
    draw_solid_group(ax, scene.get("activity_markers", []), alpha=0.85)

    (rx, ry) = candidate["room_xy"]
    (dirx, diry) = candidate["outward_xy"]
    # Aim at the room's own real centroid, not offset toward the exterior
    # wall -- an InteriorCell's own real depth band is often only a few
    # meters deep (confirmed on the real fixture: as little as ~5m), so
    # any offset large enough to matter relative to `INTERIOR_VIEW_RADIUS_M`
    # already lands past the real wall, pressing the framed box up against
    # a single wall face instead of showing the room. Centering on the
    # room itself, with `INTERIOR_VIEW_RADIUS_M` wide enough to reach past
    # the exterior wall on its own, reads as standing inside looking out.
    floor_z = candidate["floor_idx"] * FLOOR_TO_FLOOR_M
    ax.set_xlim(rx - INTERIOR_VIEW_RADIUS_M, rx + INTERIOR_VIEW_RADIUS_M)
    ax.set_ylim(ry - INTERIOR_VIEW_RADIUS_M, ry + INTERIOR_VIEW_RADIUS_M)
    # Vertical range: one real story (floor to the plate/roof above it),
    # not a fraction of the horizontal radius -- the room's own real
    # floor-to-floor height is the honest vertical extent to show, matching
    # what `interior_partition_solids` actually built at this floor.
    z_lo, z_hi = floor_z, floor_z + FLOOR_TO_FLOOR_M
    ax.set_zlim(z_lo, z_hi)
    # True proportional aspect (same technique render_isometric uses) --
    # a flat (1, 1, 1) cube here stretched the real ~3.5m vertical range to
    # match the ~18m horizontal one, which is what made the first version
    # of this shot look like it was pressed flat against a single wall.
    z_aspect = (z_hi - z_lo) / (2 * INTERIOR_VIEW_RADIUS_M)
    ax.set_box_aspect((1, 1, z_aspect))
    # Camera sits opposite the room's own outward direction, looking
    # toward it -- see this function's own docstring for mplot3d's
    # orbit-around-axis-center camera model.
    azim = math.degrees(math.atan2(-diry, -dirx))
    ax.view_init(elev=8, azim=azim)
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

    Uses the SAME `scene["buildings"]` `render_isometric` does now --
    unlike the ORIGINAL version of this function, which had to fall back
    to a separate, plain (unpunched) massing list because cadquery's GLTF
    writer re-tessellates straight from the BREP geometry with no way to
    request a coarser mesh (`Assembly.save`'s own `tolerance`/
    `angularTolerance` kwargs, and manually calling `shape.tessellate()`
    at a coarser value before `asm.add()`, both produced byte-identical
    output regardless of the value passed -- confirmed directly, not
    assumed). Measured at the time: `clean_baseline`'s 24 buildings WITH a
    real OpenCascade boolean punch per opening produced 145,344 triangles
    and a ~25.4 MiB file; the exact same 24 buildings WITHOUT punching
    produced 956 triangles and 0.17 MiB -- window/door boolean cuts were
    the dominant cost, not the building count or `scene["context"]`'s 261
    real Overture boxes (confirmed directly before re-adding those here,
    not assumed safe by analogy -- see the git history around this
    function for the full "261 context buildings alone were blamed, the
    real culprit turned out to be punching" story, since neither the
    culprit nor the false lead needs restating in full every time this
    file is read).

    Openings are no longer a boolean cut into either render path (see this
    file's own module doc for the real measured numbers behind that
    change) -- `render_isometric` draws them as flat quads, cheaply,
    outside cadquery entirely. This function does NOT also add a thin
    cadquery decal box per opening, even though that geometry would have
    been trivial to bolt on here too (same `opening_placement` numbers,
    just a `.box()` instead of a flat quad) -- tried it first, measured
    it, reverted it (no trace of that attempt is left in this file other
    than this note -- the code itself was deleted, not commented out):
    `clean_baseline`'s 35 buildings carry
    7,765 real openings, and 7,765 real decal boxes (~93,180 triangles,
    grouped into ONE `Compound` per color to rule out per-node glTF
    overhead as the cause) still produced a ~28.5 MiB file in ~36s --
    over Cloudflare Workers' 25 MiB hard cap, the exact failure mode this
    function's own history already documents for a different cause (see
    the git history around this function for that "261 context buildings
    alone were blamed, the real culprit turned out to be punching" story).
    Real per-opening 3D geometry is simply too much data at this
    fixture's real opening density for a 25 MiB budget, whether it's cut
    out of the wall, added as separate solids, or added as one merged
    compound -- a `Compound` only removes per-node metadata overhead, not
    the underlying triangle count, and the triangle count was always the
    real cost (same lesson `punch_openings`'s own removal already
    established). So the GLB stays exactly what it was before this file's
    P124/decal work: plain massing, no window/door detail -- only the
    isometric PNG, where a decal costs a numpy quad instead of a real OCC
    solid, gets that detail.

    `scene["interior_walls"]` is the one exception, and only when a caller
    opted into `build_scene`'s `include_interior_walls` -- real, additive
    partition-wall extrusions (a handful of solids on a single showcase
    cluster, nothing like the 7,765-opening budget problem above) with the
    `buildings` group's own alpha lowered to read as a translucent shell
    around them. Every existing scenario leaves `interior_walls` empty, so
    this GLB stays byte-for-byte what it always was for them. See
    `render_floor_plan`'s own module doc for why a full FLOOR PLAN is still
    drawn as 2D, not built into any 3D solid -- this is a different, much
    smaller real geometry (single-story partition walls), not that.
    """
    asm = cq.Assembly()
    n_added = 0
    interior_walls = scene.get("interior_walls", [])
    buildings_alpha = 0.35 if interior_walls else 0.82
    for group, alpha, solids_key in (
        ("streets", 0.55, "streets"), ("plazas", 0.6, "plazas"), ("buildings", buildings_alpha, "buildings"),
        ("interior_walls", 0.95, "interior_walls"),
        ("activity", 0.95, "activity_markers"),
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
    section through the result (a real OpenCascade boolean, the same
    technique this file's original opening-punch approach once used for
    exterior openings -- see this file's own module doc for why that was
    later replaced with a flat decal -- just adding material instead of
    subtracting it here).
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

    Known real gap since `build_scene`'s own P124 pocket refill was added:
    every floor's outline here is drawn from `b["polygon"]["outer"]` (the
    one, already-notched ring this schema has) -- so a building with a
    real pocket shows the notch in EVERY 2D panel, ground floor through
    roof, while the 3D massing (which fills the notch back in above ground
    level) does not. No 2D per-floor footprint exists to draw instead; this
    is a known, not-yet-fixed disagreement between the two views, same
    honesty-over-silence spirit as this file's other documented gaps.
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
    # `--interiors` is a bare flag, not a positional -- stripped out before
    # the existing positional parsing below so it can appear anywhere on
    # the command line without shifting `context_buildings.geojson`.
    argv = sys.argv[1:]
    include_interior_walls = "--interiors" in argv
    if include_interior_walls:
        argv = [a for a in argv if a != "--interiors"]
    if len(argv) not in (2, 3):
        print("usage: render.py <neighborhood.json> <output_prefix> [context_buildings.geojson] [--interiors]")
        sys.exit(1)
    nbhd_path, out_prefix = argv[0], argv[1]
    context_path = argv[2] if len(argv) == 3 else None
    nbhd = load(nbhd_path)
    print(f"=== {nbhd_path} ===")

    # Coarse per-phase profiling (see build_scene's own finer breakdown of
    # its own time) -- throwaway stderr output, not a test or a stored
    # baseline, added specifically because a single before/after
    # end-to-end wall-clock comparison already turned out to be swamped by
    # +/-50s sandbox noise (see the P124 refill work's own commit
    # message) -- phase attribution is the only way to tell real cost from
    # noise in this environment.
    _t0 = time.perf_counter()
    scene = build_scene(nbhd, context_path=context_path, include_interior_walls=include_interior_walls)
    _t_build_scene = time.perf_counter() - _t0
    print(
        f"buildings: {len(scene['buildings'])}, plazas: {len(scene['plazas'])}, "
        f"streets: {len(scene['streets'])}, activity_markers: {len(scene.get('activity_markers', []))}, "
        f"interior_walls: {len(scene.get('interior_walls', []))}, context: {len(scene['context'])}"
    )

    _t0 = time.perf_counter()
    render_isometric(scene, f"{out_prefix}_isometric.png", nbhd_path.split("/")[-1])
    _t_isometric = time.perf_counter() - _t0

    origin_lng, origin_lat = scene["origin"]
    _t0 = time.perf_counter()
    render_floor_plan(nbhd, origin_lng, origin_lat, 0, f"{out_prefix}_floorplan_ground.svg", "floor plan (ground)")
    max_floors = max((b.get("floors") or 1) for b in nbhd.get("buildings", [])) if nbhd.get("buildings") else 1
    if max_floors >= 2:
        render_floor_plan(
            nbhd, origin_lng, origin_lat, 1, f"{out_prefix}_floorplan_upper.svg", "floor plan (floor 2)"
        )
    render_largest_building_floors(nbhd, origin_lng, origin_lat, f"{out_prefix}_floorplan_largest_building.svg")
    _t_floorplans = time.perf_counter() - _t0

    _t0 = time.perf_counter()
    export_glb(scene, f"{out_prefix}.glb")
    _t_glb = time.perf_counter() - _t0

    _t0 = time.perf_counter()
    n_interior_views = 0
    if include_interior_walls:
        candidates = interior_view_candidates(nbhd, origin_lng, origin_lat)
        for i, candidate in enumerate(candidates):
            render_interior_view(
                scene, candidate, f"{out_prefix}_interior_view_{i}.png",
                f"{candidate['building']['id']} -- inside looking out",
            )
            n_interior_views += 1
        # Real <model-viewer> camera-target/camera-orbit numbers for the
        # SAME real rooms, so the gallery page can place a real hardware-
        # accelerated WebGL camera inside the interactive .glb instead of
        # relying only on this file's own flat mplot3d renders above --
        # see interior_camera_views' own docstring for the coordinate math.
        with open(f"{out_prefix}_interior_views.json", "w") as f:
            json.dump(interior_camera_views(candidates), f, indent=2)
    _t_interior_views = time.perf_counter() - _t0

    print(
        f"phase timing: build_scene={_t_build_scene:.1f}s isometric={_t_isometric:.1f}s "
        f"floorplans={_t_floorplans:.1f}s glb={_t_glb:.1f}s interior_views({n_interior_views})={_t_interior_views:.1f}s "
        f"total={_t_build_scene + _t_isometric + _t_floorplans + _t_glb + _t_interior_views:.1f}s"
    )


if __name__ == "__main__":
    main()
