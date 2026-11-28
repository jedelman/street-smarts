"""
lights.py: Ghent's streetlights, where the City's data puts them. Writes data/ghent/lights.json; build.py puts them in
the model and the walk draws them (public/sparrow/lamps.js).

Source: City of Norfolk, "Streetlights and Outages Reports" (data.norfolk.gov dataset 5qpt-tdve): "Dominion Energy
owned (city leased) streetlight locations and types", collected and maintained by Dominion Energy, updated weekly.
Only each light's position, fixture style and pole material are requested: never a work order, its status or who
reported an outage.

The dataset gives fixture styles as codes (TRAD, COLN, COXX, ...) with no key. FAMILY reads them against the styles
Dominion's own Virginia catalogue names (Colonial, Acorn, Cobra, Shoebox, Directional Flood): that reading is ours.
Rows repeat a light once per outage report, so lights are kept once per position.

Run (network): tools/vibe-render/.venv/bin/python tools/ghent/lights.py
"""

import json
import os
import sys

from shapely.geometry import Point, Polygon
from shapely.ops import unary_union

sys.path.insert(0, os.path.dirname(__file__))
import survey  # noqa: E402

ROOT = survey.ROOT
OUT = os.path.join(ROOT, "data/ghent/lights.json")
DATASET = "https://data.norfolk.gov/resource/5qpt-tdve.json"
RING_M = 60  # lights just past the League's edge still light its streets

# Our reading of the codes, by the style names in Dominion's catalogue.
FAMILY = {
    "TRAD": "colonial", "COLN": "colonial", "CLTC": "colonial", "CLCO": "colonial", "CLSU": "colonial",
    "ACRN": "acorn", "ACBA": "acorn", "ACCA": "acorn",
    "COXX": "cobra", "SBBA": "shoebox",
}
POLES = {"Concrete": "concrete", "Fiberglass": "fiberglass", "Aluminum": "aluminum", "Steel": "steel", "Wood": "wood"}


def main():
    S = json.load(open(os.path.join(ROOT, "data/ghent/survey.json")))
    league = unary_union([Polygon(r) for r in S["league"]])
    reach = league.buffer(RING_M)
    A, B = S["meta"]["to_latlng"]["lat"], S["meta"]["to_latlng"]["lng"]
    det = A[0] * B[1] - A[1] * B[0]

    def to_local(lat, lng):
        a, b = lat - A[2], lng - B[2]
        return ((a * B[1] - A[1] * b) / det, (A[0] * b - a * B[0]) / det)
    corners = [(A[0] * x + A[1] * y + A[2], B[0] * x + B[1] * y + B[2]) for x, y in reach.exterior.coords]
    s, n = min(c[0] for c in corners), max(c[0] for c in corners)
    w, e = min(c[1] for c in corners), max(c[1] for c in corners)
    rows = survey.cached_json(DATASET, {"$select": "fixture_style,pole_material,latitude,longitude",
                                        "$where": f"latitude between {s} and {n} and longitude between {w} and {e}",
                                        "$limit": 50000})
    seen, lights, codes = set(), [], {}
    for r in rows:
        if not r.get("latitude") or not r.get("longitude"):
            continue
        key = (round(float(r["latitude"]), 6), round(float(r["longitude"]), 6))
        if key in seen:
            continue
        seen.add(key)
        x, y = to_local(*key)
        if not reach.contains(Point(x, y)):
            continue
        code = (r.get("fixture_style") or "").strip().upper()
        codes[code] = codes.get(code, 0) + 1
        lights.append([round(x, 2), round(y, 2), FAMILY.get(code, "other"), POLES.get(r.get("pole_material"), "other"), code])
    lights.sort()
    out = {"version": 1, "source": "City of Norfolk, Streetlights and Outages Reports (data.norfolk.gov 5qpt-tdve), "
           "Dominion Energy's data", "families": "our reading of the fixture codes against Dominion's catalogue of styles",
           "codes": dict(sorted(codes.items(), key=lambda kv: -kv[1])),
           # [x, y (survey metres), family, pole, code]
           "lights": lights}
    with open(OUT, "w") as f:
        json.dump(out, f, separators=(",", ":"))
    fam = {}
    for l in lights:
        fam[l[2]] = fam.get(l[2], 0) + 1
    print(f"lights.json: {len(lights)} lights {fam}")


if __name__ == "__main__":
    main()
