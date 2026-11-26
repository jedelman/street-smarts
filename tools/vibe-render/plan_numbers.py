#!/usr/bin/env python3
"""Measure what a rendered plan adds up to, for the landing page's numbers.

Reads the pipeline's own output (the neighborhood JSON) and the packed GLB
it was rendered into (for the shop fronts render.py found), and writes a
small JSON of plain measurements: buildings, floor area, upper-floor area,
floor area by use (P9 Scattered Work's live / work / play),
shop fronts and their frontage, trees, open space, the light rail's station.
No assumptions live here. The landing page's rough economics (homes per
square foot, spending per household, sales per square foot) are stated and
sourced where they're used, in public/index.html, so that every number this
script writes is measured and every number it doesn't is labeled.

    python plan_numbers.py target/vibe-render/clean_baseline.json \\
        target/vibe-render/clean_baseline.glb > target/vibe-render/clean_baseline_numbers.json
"""

import json
import math
import statistics
import sys

from pack_glb import read_glb


def ring_area_m2(ring):
    """Area of a closed WGS84 ring in square metres (local equirectangular; fine at a site's scale)."""
    if len(ring) < 3:
        return 0.0
    lat0 = math.radians(sum(p["lat"] for p in ring) / len(ring))
    xs = [p["lng"] * 111_320.0 * math.cos(lat0) for p in ring]
    ys = [p["lat"] * 110_540.0 for p in ring]
    return abs(sum(xs[i] * ys[(i + 1) % len(xs)] - xs[(i + 1) % len(xs)] * ys[i] for i in range(len(xs)))) / 2.0


def polygon_area_m2(poly):
    return max(0.0, ring_area_m2(poly["outer"]) - sum(ring_area_m2(h) for h in poly.get("holes") or []))


def line_length_m(points):
    lat0 = math.radians(points[0]["lat"])
    return sum(
        math.hypot((b["lng"] - a["lng"]) * 111_320.0 * math.cos(lat0), (b["lat"] - a["lat"]) * 110_540.0)
        for a, b in zip(points, points[1:])
    )


def main(nbhd_path, glb_path):
    n = json.load(open(nbhd_path))
    gltf, _ = read_glb(glb_path)
    table = (gltf.get("nodes") or [{}])[0].get("extras", {}).get("streetSmarts") or {}
    shops = ((table.get("detail") or {}).get("shops") or {}).get("rows") or []

    footprint = floor_area = 0.0
    floors = []
    by_use = {"live": 0.0, "work": 0.0, "play": 0.0}
    for b in n["buildings"]:
        a = polygon_area_m2(b["polygon"])
        f = int(b.get("floors") or 1)
        footprint += a
        floor_area += a * f
        floors.append(f)
        # P9's floor uses; without them, the old reading: shops at street level, homes above.
        uses = b.get("floor_uses") or (["play"] + ["live"] * (f - 1))
        for u in uses:
            by_use[u] = by_use.get(u, 0.0) + a
    widths = [row[2] - row[1] for row in shops]  # hi - lo, metres along the wall
    open_space = sum(polygon_area_m2(o["polygon"]) for o in n.get("open_space") or [])
    link = next((s for s in n.get("streets") or [] if s["id"].endswith("_p34_link")), None)
    stations = sum(len(t.get("stations") or []) for t in n.get("transit") or [])

    out = {
        "version": 1,
        "buildings": len(n["buildings"]),
        "floors_median": statistics.median(floors) if floors else 0,
        "floors_max": max(floors) if floors else 0,
        "footprint_m2": round(footprint),
        "floor_area_m2": round(floor_area),
        "upper_floor_area_m2": round(floor_area - footprint),
        "live_m2": round(by_use["live"]),
        "work_m2": round(by_use["work"]),
        "play_m2": round(by_use["play"]),
        "uses_assigned": any(b.get("floor_uses") for b in n["buildings"]),
        "shop_fronts": len(shops),
        "p87_shop_fronts": sum(1 for row in shops if row[7]),
        "frontage_m": round(sum(widths)),
        "shop_front_median_m": round(statistics.median(widths), 2) if widths else 0,
        "trees": len(n.get("trees") or []),
        "open_space_m2": round(open_space),
        "transit_stations": stations,
        "station_path_m": round(line_length_m(link["centerline"])) if link else None,
    }
    json.dump(out, sys.stdout, indent=1)
    print()


if __name__ == "__main__":
    main(*sys.argv[1:3])
