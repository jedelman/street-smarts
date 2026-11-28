"""
water.py: open water in and around the Ghent Neighborhood League's area, where the City's GIS maps it. Writes
data/ghent/water.json; build.py draws it (below the land, behind a bank) and keeps lots, sidewalks and people off it.

Source: City of Norfolk GIS Bureau, "Water" (gisshare.norfolk.gov OpenData/Environmental/FeatureServer/0): "polygon
features of water bodies". Only each polygon's geometry and HYDRO_ID are requested. The layer carries no names: the
one water body inside the League's area is the Hague (Jason: "let's put water in the Hague"), so that one is named;
any other is just water. Kept within the model's context ring (survey.CONTEXT_RING_FT) of the League's area.

Bridges: Overture Maps Foundation's transportation segments (release OVERTURE_RELEASE, from OpenStreetMap, ODbL), the
stretches of road and path flagged is_bridge that cross this water: the Elizabeth River Trail's footbridge over the
Hague and the roads that cross it. Their lines are Overture's; how wide build.py draws their decks is ours.

Run (network; duckdb only to refresh Overture, and without ambient AWS credentials, which S3's anonymous reads refuse):
    env -u AWS_ACCESS_KEY_ID -u AWS_SECRET_ACCESS_KEY tools/vibe-render/.venv/bin/python tools/ghent/water.py
"""

import json
import os
import sys

from shapely.geometry import LineString, Polygon
from shapely.geometry.polygon import orient
from shapely.ops import substring, transform, unary_union

sys.path.insert(0, os.path.dirname(__file__))
import survey  # noqa: E402

ROOT = survey.ROOT
OUT = os.path.join(ROOT, "data/ghent/water.json")
LAYER = "https://gisshare.norfolk.gov/pubserver/rest/services/OpenData/Environmental/FeatureServer/0"
OVERTURE_RELEASE = "2026-11-18.0"  # the release everyday.py reads


def overture_bridges(box):
    """Overture's road segments with a stretch flagged is_bridge in the box (lat/lng): [class, name, [[lng, lat], ...],
    [from, to] fractions or None]. Cached as JSON."""
    key = os.path.join(survey.CACHE, f"overture_bridges_{OVERTURE_RELEASE}.json")
    if os.path.exists(key):
        return json.load(open(key))
    import duckdb  # only to refresh: pip install duckdb
    c = duckdb.connect()
    c.execute("INSTALL spatial; LOAD spatial; INSTALL httpfs; LOAD httpfs; SET s3_region='us-west-2';")
    src = f"read_parquet('s3://overturemaps-us-west-2/release/{OVERTURE_RELEASE}/theme=transportation/type=segment/*', hive_partitioning=1)"
    rows = c.execute(f"""SELECT class, names.primary, road_flags, ST_AsGeoJSON(geometry) FROM {src}
                         WHERE subtype = 'road' AND road_flags IS NOT NULL AND bbox.xmin < {box['e']} AND bbox.xmax > {box['w']}
                         AND bbox.ymin < {box['n']} AND bbox.ymax > {box['s']}""").fetchall()
    out = []
    for cls, name, flags, gj in rows:
        for f in flags or []:
            if "is_bridge" in (f.get("values") or []):
                out.append([cls, name, json.loads(gj)["coordinates"], f.get("between")])
    os.makedirs(survey.CACHE, exist_ok=True)
    json.dump(out, open(key, "w"))
    return out


def main():
    S = json.load(open(os.path.join(ROOT, "data/ghent/survey.json")))
    cx, cy = S["meta"]["centre_ft"]
    league = unary_union([Polygon(r) for r in S["league"]])
    reach = league.buffer(survey.CONTEXT_RING_FT * survey.FT)
    minx, miny, maxx, maxy = reach.bounds
    env = (cx + minx / survey.FT, cy + miny / survey.FT, cx + maxx / survey.FT, cy + maxy / survey.FT)
    feats = survey.arcgis(LAYER, envelope=env, out_fields="HYDRO_ID")
    bodies = []
    for f in feats:
        g = survey.geom(f)
        if g is None:
            continue
        local = transform(lambda x, y, z=None: ((x - cx) * survey.FT, (y - cy) * survey.FT), g).buffer(0).intersection(reach)
        parts = [orient(p.simplify(0.4), 1.0) for p in survey.polys(local) if p.area > 20]
        if not parts:
            continue
        inside = sum(p.intersection(league).area for p in parts)
        bodies.append({"id": f["properties"].get("HYDRO_ID"), "inside": inside, "parts": parts})
    bodies.sort(key=lambda b: -b["inside"])
    out = {"version": 1,
           "source": "City of Norfolk GIS Bureau, Water (gisshare.norfolk.gov OpenData/Environmental/FeatureServer/0)",
           "note": "The layer has no names; the water body inside the League's area is named the Hague here.",
           # each: its HYDRO_ID, a name, and polygons as [exterior, hole, ...] rings of [x, y] survey metres
           "bodies": []}
    ring = lambda r: [[round(x, 2), round(y, 2)] for x, y in list(r.coords)[:-1]]
    for i, b in enumerate(bodies):
        # Only the part that reaches into the League's area is the Hague; the layer's other parts past the edge may
        # be other water (it maps the river system as one feature), so they stay unnamed.
        named = [p for p in b["parts"] if i == 0 and p.intersection(league).area > 1000]
        rest = [p for p in b["parts"] if p not in named]
        if named:
            out["bodies"].append({"id": b["id"], "name": "the Hague", "polygons": [[ring(r) for r in [p.exterior, *p.interiors]] for p in named]})
        if rest:
            out["bodies"].append({"id": b["id"], "name": None, "polygons": [[ring(r) for r in [p.exterior, *p.interiors]] for p in rest]})
    # Bridges over this water, from Overture: the bridged stretch of each segment, in survey metres.
    A, B = S["meta"]["to_latlng"]["lat"], S["meta"]["to_latlng"]["lng"]
    det = A[0] * B[1] - A[1] * B[0]

    def to_local(lng, lat):
        a, b = lat - A[2], lng - B[2]
        return ((a * B[1] - A[1] * b) / det, (A[0] * b - a * B[0]) / det)
    corners = [(A[0] * x + A[1] * y + A[2], B[0] * x + B[1] * y + B[2]) for x, y in reach.exterior.coords]
    box = {"s": min(c[0] for c in corners), "n": max(c[0] for c in corners), "w": min(c[1] for c in corners), "e": max(c[1] for c in corners)}
    water = unary_union([Polygon(p[0], p[1:]) for b in out["bodies"] for p in b["polygons"]])
    out["bridges_source"] = (f"Overture Maps Foundation, transportation, release {OVERTURE_RELEASE} (from OpenStreetMap, ODbL): "
                             "road and path segments flagged is_bridge")
    out["bridges"] = []
    for cls, name, coords, between in overture_bridges(box):
        line = LineString([to_local(lng, lat) for lng, lat in coords])
        part = substring(line, between[0], between[1], normalized=True) if between else line
        if part.length < 3 or not part.intersects(water):
            continue
        out["bridges"].append({"class": cls, "name": name, "line": [[round(x, 2), round(y, 2)] for x, y in part.coords]})
    out["bridges"].sort(key=lambda b: (b["class"] or "", b["name"] or "", b["line"]))
    with open(OUT, "w") as fh:
        json.dump(out, fh, separators=(",", ":"))
        fh.write("\n")
    for b in out["bodies"]:
        area = sum(Polygon(p[0], p[1:]).area for p in b["polygons"])
        print(f"water {b['id']}: {b['name'] or 'unnamed'}, {len(b['polygons'])} part(s), {area:,.0f} m2")
    for b in out["bridges"]:
        print(f"bridge: {b['class']} {b['name'] or ''}, {LineString(b['line']).length:.0f} m")


if __name__ == "__main__":
    main()
