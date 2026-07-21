#!/usr/bin/env python3
"""Extract one real building CLUSTER from a full site Neighborhood JSON --
a scoped-down JSON render.py can run exactly as-is, for a closer, more
detailed look at one real group of buildings instead of the whole site.

# What counts as a "cluster" here

There's no single field that names a real cluster: `p108_connected_buildings`
merges pads into `p108_merged_N_building`, discarding which real P37 block
they came from (see its own module doc -- pads merge when "very close,"
which in practice means the same block, but that's never verified or
recorded). A raw, unmerged P95 pad DOES carry its block in its own id
(`<BLOCK>_P95_cell_N_building`), but that convention breaks the moment a
building is `p108_merged_*` instead. So this tool doesn't parse ids at
all -- it uses real geometry: pick an ANCHOR building (by id, or the
building with the most real neighbors within `radius_m` if none given),
then keep every building whose own centroid sits within `radius_m` of the
anchor's centroid. A real, honest proxy for "one building complex," not a
guess at a naming convention that doesn't hold uniformly.

Real open space / streets / activity nodes are kept too, by the same
real-distance test against their own centroid -- so a cluster's own
plaza, pockets, and connecting paths render alongside it, not just bare
building massing with no site context.

Usage:
    extract_cluster.py <full.json> <out.json> [--anchor BUILDING_ID] [--radius-m 60]
"""
import argparse
import json
import math
import sys


def centroid(ring):
    xs = [p["lng"] for p in ring]
    ys = [p["lat"] for p in ring]
    return sum(xs) / len(xs), sum(ys) / len(ys)


def dist_m(a, b):
    lat0 = math.radians((a[1] + b[1]) / 2)
    dx = (a[0] - b[0]) * 111_320.0 * math.cos(lat0)
    dy = (a[1] - b[1]) * 110_540.0
    return math.hypot(dx, dy)


def pick_densest_anchor(building_centroids, radius_m):
    """The building with the most real neighbors within radius_m -- a
    real, non-arbitrary choice when the caller doesn't name one."""
    best_id, best_n = None, -1
    for bid, c in building_centroids.items():
        n = sum(1 for oid, oc in building_centroids.items() if oid != bid and dist_m(c, oc) <= radius_m)
        if n > best_n:
            best_id, best_n = bid, n
    return best_id


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("input")
    ap.add_argument("output")
    ap.add_argument("--anchor", default=None, help="Building id to center the cluster on. Default: the building with the most real neighbors within --radius-m.")
    ap.add_argument("--radius-m", type=float, default=60.0, help="Real distance (meters) from the anchor's own centroid a building/open-space/street must sit within to be kept.")
    args = ap.parse_args()

    with open(args.input) as f:
        nbhd = json.load(f)

    building_centroids = {b["id"]: centroid(b["polygon"]["outer"]) for b in nbhd["buildings"] if b["polygon"]["outer"]}
    if not building_centroids:
        print("! no real buildings in the input -- nothing to cluster", file=sys.stderr)
        sys.exit(1)

    anchor_id = args.anchor or pick_densest_anchor(building_centroids, args.radius_m)
    if anchor_id not in building_centroids:
        print(f"! anchor '{anchor_id}' not found among real building ids", file=sys.stderr)
        sys.exit(1)
    anchor_c = building_centroids[anchor_id]

    kept_buildings = [b for b in nbhd["buildings"] if b["id"] in building_centroids and dist_m(building_centroids[b["id"]], anchor_c) <= args.radius_m]
    kept_ids = {b["id"] for b in kept_buildings}

    def within(ring):
        if not ring:
            return False
        return dist_m(centroid(ring), anchor_c) <= args.radius_m

    kept_open_space = [o for o in nbhd.get("open_space", []) if within(o["polygon"]["outer"])]
    kept_streets = [s for s in nbhd.get("streets", []) if s.get("centerline") and dist_m(
        (sum(p["lng"] for p in s["centerline"]) / len(s["centerline"]), sum(p["lat"] for p in s["centerline"]) / len(s["centerline"])),
        anchor_c,
    ) <= args.radius_m]
    kept_activity = [a for a in nbhd.get("activity_nodes", []) if dist_m((a["location"]["lng"], a["location"]["lat"]), anchor_c) <= args.radius_m]

    # render.py's own build_scene() does two real things with `parcels`
    # beyond just siting real Buildings: site_parcels() (BLOCK_/P95_ specs)
    # sets the render origin, AND a real fallback loop renders every
    # "p95_building_pad"/"p95_pad_with_building" parcel that ISN'T already
    # a shaped Building as a plain massing box -- so leaving `parcels`
    # as the whole site's own would pull in real, unrelated pads from
    # every OTHER cluster too. Filtered by the same real distance test.
    kept_parcels = [p for p in nbhd.get("parcels", []) if within(p["polygon"]["outer"])]

    out = dict(nbhd)
    out["buildings"] = kept_buildings
    out["open_space"] = kept_open_space
    out["streets"] = kept_streets
    out["parcels"] = kept_parcels
    out["activity_nodes"] = kept_activity
    # Parcels/boundaries aren't consumed by render.py's own building/open-
    # space/street path -- left untouched (still the full site's parcels)
    # since render.py only reads site_parcels() for its own origin
    # calculation, not per-cluster filtering, and trimming them isn't
    # needed for a real, correct cluster render.

    with open(args.output, "w") as f:
        json.dump(out, f)

    print(f"cluster anchor={anchor_id} radius_m={args.radius_m}: {len(kept_buildings)} building(s) "
          f"({', '.join(sorted(kept_ids))}), {len(kept_open_space)} open-space, {len(kept_streets)} street(s)")


if __name__ == "__main__":
    main()
