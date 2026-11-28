"""
everyday.py: a day in Ghent, in walking minutes and dollars, from the sources that say so. It writes
data/ghent/everyday.json, which build.py joins to the proposal for the walk's "Day to day" panel.

  - The streets to walk: the City's street centerlines (gisshare.norfolk.gov, OpenData/Public_Works/FeatureServer/1),
    out to WALK_RING_M past the League's edge, so a walk from its edge isn't cut short. Limited-access roads and
    ramps are left out. Distances run along centerlines: crossings and sidewalks aren't modelled.
  - The places already open: Overture Maps Foundation's places (release OVERTURE_RELEASE, CDLA Permissive 2.0),
    those marked open with confidence of at least MIN_CONFIDENCE, and only the kinds in KINDS. No names are kept:
    a place is a kind at a point on the street network.
  - What has opened: the City's new business licences (data.norfolk.gov dataset dpi6-sct5: "all new business
    licenses issued by the Commissioner of the Revenue and opened from January 1, 2019 to present"; it has no
    renewals and doesn't say which businesses have closed). Only the kind (from its NAICS description) and the
    opening date are requested; never the name, the owner or the address. Each is counted at the street-network
    corner nearest its geocoded point, so no record is kept at a door.
  - Rent: the Census Bureau's American Community Survey, 2020-2024 5-year estimates (tables B25064 median gross
    rent, B25003 tenure, B25070 gross rent as a share of income), for the census tracts the League's area touches
    (TIGERweb), read through Census Reporter's API (api.censusreporter.org), which serves the Bureau's tables.
  - How often a place is new: the Census Bureau's Business Dynamics Statistics, 2023 release (bds2023_msa_sec.csv),
    establishment entry and exit rates for the Virginia Beach-Norfolk metro area (CBSA 47260), 2019-2023, for retail
    (NAICS 44-45), accommodation and food services (72) and other services (81).

Run (network; duckdb only to refresh Overture):
    tools/vibe-render/.venv/bin/python tools/ghent/everyday.py
"""

import csv
import datetime
import io
import json
import math
import os
import re
import sys
import urllib.request

from shapely.geometry import LineString, Point, Polygon, shape
from shapely.ops import unary_union
import shapely

sys.path.insert(0, os.path.dirname(__file__))
import survey  # noqa: E402  (arcgis, cached_json, the City's layers)

ROOT = survey.ROOT
SURVEY = os.path.join(ROOT, "data/ghent/survey.json")
OUT = os.path.join(ROOT, "data/ghent/everyday.json")
FT = survey.FT
WALK_RING_M = 1300            # a 15-minute walk at WALK_MPS, plus slack
WALK_MPS = 1.34               # 3 miles an hour (ours)
OVERTURE_RELEASE = "2026-11-18.0"
MIN_CONFIDENCE = 0.6
LICENCES = "https://data.norfolk.gov/resource/dpi6-sct5.json"
TRACTS = "https://tigerweb.geo.census.gov/arcgis/rest/services/TIGERweb/tigerWMS_ACS2024/MapServer/8"
ACS = "https://api.censusreporter.org/1.0/data/show/acs2024_5yr"
BDS = "https://www2.census.gov/programs-surveys/bds/tables/time-series/2023/bds2023_msa_sec.csv"
CBSA = "47260"

# The kinds a neighbour walks to: Overture's hierarchy words that make a place one, the proposal's businesses that
# are one, and what to call it. Order matters: a bakery is a casual eatery too, so it's tested before "meal".
KINDS = [
    ("coffee", "Coffee", {"coffee_shop", "cafe", "tea_room", "bubble_tea_shop"}, {"coffee"}),
    ("bakery", "Bread and pastry", {"bakery", "patisserie_cake_shop", "bagel_shop", "donut_shop", "cupcake_shop"}, {"bakery"}),
    ("meal", "A meal out", {"restaurant", "casual_eatery", "alcoholic_beverage_venue"}, {"bistro"}),
    ("groceries", "Groceries", {"grocery_store", "butcher_shop", "fishmonger", "specialty_foods_store", "health_food_store",
                                "farmers_market", "produce_store"}, {"greengrocer"}),
    ("wine", "Wine and beer", {"liquor_store", "beer_wine_spirits_store", "wine_store"}, {"wine"}),
    ("books", "Books", {"bookstore"}, {"books"}),
    ("flowers", "Flowers and gifts", {"florist", "flowers_and_gifts_store"}, {"florist"}),
    ("hardware", "Hardware", {"hardware_store", "paint_store"}, {"hardware"}),
    ("bikes", "A bike shop", {"bike_store"}, {"bikes"}),
    ("records", "Records", {"vinyl_record_store", "music_and_dvd_store"}, {"records"}),
    ("hair", "A haircut", {"hair_salon", "barber", "hair_stylist"}, {"hairdresser", "barber"}),
    ("cleaners", "Dry cleaning", {"dry_cleaning", "laundry_service"}, {"cleaners"}),
    ("tailor", "Alterations", {"sewing_and_alterations"}, {"tailor"}),
    ("dentist", "A dentist", {"dental_clinic"}, {"dentist"}),
    ("gallery", "An art gallery", {"art_gallery"}, {"gallery"}),
    ("fitness", "Yoga or a gym", {"gym", "fitness_studio"}, {"yoga"}),
    ("pharmacy", "A pharmacy", {"pharmacy_and_drug_store"}, set()),
]
NOT_A_MEAL = {"dessert_shop", "ice_cream_shop", "candy_store", "frozen_yogurt_shop", "gelato_shop"}
NOT_FLOWERS = {"gift_shop"}

# Licences by kind, from the City's NAICS descriptions: the sorts of place a neighbour walks to.
LICENCE_SECTORS = [
    ("food", re.compile(r"restaurant|drinking|\bbars?\b|brewer|winer|bakery|bakeries|coffee|snack", re.I)),
    ("shop", re.compile(r"retail|\bstores?\b|florist|grocer|\bbook(s|store|seller)|hardware|bicycle|jewel|clothing|antique|"
                        r"\bgifts?\b|\bmarkets?\b", re.I)),
    ("service", re.compile(r"beaut|barber|salon|nail|hair|laundr|dry.?clean|tailor|alteration|fitness|yoga|massage|"
                           r"art dealer|gallery|dentist|tattoo|spa\b|shoe repair", re.I)),
]
NOT_A_STOREFRONT = re.compile(r"web|mail order|internet|nonstore|non-store|direct sell|vending|peddl|special event|"
                              r"wholesale|manufactur|home health|mobile|gas station|caterer|consult|bookkeep", re.I)


def kind_of(hierarchy):
    h = set(hierarchy or [])
    for kid, _, words, _ in KINDS:
        if h & words:
            if kid == "meal" and h & NOT_A_MEAL:
                return None
            if kid == "flowers" and h & NOT_FLOWERS and "florist" not in h:
                return None
            return kid
    return None


def licence_sector(naics):
    if not naics or NOT_A_STOREFRONT.search(naics):
        return None
    for sec, rx in LICENCE_SECTORS:
        if rx.search(naics):
            return sec
    return None


def fetch_text(url):
    key = os.path.join(survey.CACHE, "bds_" + os.path.basename(url))
    if os.path.exists(key):
        return open(key).read()
    with urllib.request.urlopen(urllib.request.Request(url, headers=survey.UA), timeout=180) as r:
        text = r.read().decode("utf-8")
    os.makedirs(survey.CACHE, exist_ok=True)
    open(key, "w").write(text)
    return text


def overture_places(box):
    """Overture's places in the box (lat/lng), as (lng, lat, hierarchy, confidence, status). Cached as JSON."""
    key = os.path.join(survey.CACHE, f"overture_places_{OVERTURE_RELEASE}.json")
    if os.path.exists(key):
        return json.load(open(key))
    import duckdb  # only to refresh: pip install duckdb
    c = duckdb.connect()
    c.execute("INSTALL httpfs; LOAD httpfs; SET s3_region='us-west-2';")
    src = f"read_parquet('s3://overturemaps-us-west-2/release/{OVERTURE_RELEASE}/theme=places/type=place/*', hive_partitioning=1)"
    # A place is a point, so its bbox's corners are the point.
    rows = c.execute(f"""SELECT bbox.xmin, bbox.ymin, taxonomy.hierarchy, confidence, operating_status FROM {src}
                         WHERE bbox.xmin BETWEEN {box['w']} AND {box['e']} AND bbox.ymin BETWEEN {box['s']} AND {box['n']}""").fetchall()
    out = [[r[0], r[1], list(r[2] or []), r[3], r[4]] for r in rows]
    os.makedirs(survey.CACHE, exist_ok=True)
    json.dump(out, open(key, "w"))
    return out


def main():
    S = json.load(open(SURVEY))
    league = unary_union([Polygon(r) for r in S["league"]])
    reach = league.buffer(WALK_RING_M)
    A, B = S["meta"]["to_latlng"]["lat"], S["meta"]["to_latlng"]["lng"]
    det = A[0] * B[1] - A[1] * B[0]

    def to_local(lat, lng):
        a, b = lat - A[2], lng - B[2]
        return ((a * B[1] - A[1] * b) / det, (A[0] * b - a * B[0]) / det)

    def to_latlng(x, y):
        return A[0] * x + A[1] * y + A[2], B[0] * x + B[1] * y + B[2]

    corners = [to_latlng(x, y) for x, y in reach.exterior.coords]
    box = {"s": min(c[0] for c in corners), "n": max(c[0] for c in corners),
           "w": min(c[1] for c in corners), "e": max(c[1] for c in corners)}
    cx, cy = S["meta"]["centre_ft"]

    # 1. The streets to walk.
    minx, miny, maxx, maxy = reach.bounds
    env = (cx + minx / FT, cy + miny / FT, cx + maxx / FT, cy + maxy / FT)
    segs, skipped = [], set()
    for f in survey.arcgis(survey.STREETS, envelope=env, out_fields="FULLNAME,ROW"):
        g = survey.geom(f)
        row, name = (f["properties"].get("ROW") or ""), (f["properties"].get("FULLNAME") or "")
        if not g:
            continue
        if re.search(r"limited|freeway|interstate|ramp|expressway", row, re.I) or re.search(r"\bRAMP\b|^I-?\d|INTERSTATE", name, re.I):
            skipped.add(row or name)
            continue
        for ln in survey.lines(g):
            pts = [((x - cx) * FT, (y - cy) * FT) for x, y in ln.coords]
            if len(pts) >= 2:
                segs.append(LineString(pts).simplify(0.5))
    noded = unary_union([s for s in segs if s.intersects(reach)])
    nodes, index, edges = [], {}, []

    def node(p):
        k = (round(p[0] * 2), round(p[1] * 2))  # half a metre
        if k not in index:
            index[k] = len(nodes)
            nodes.append([round(p[0], 1), round(p[1], 1)])
        return index[k]
    for ln in survey.lines(noded):
        cs = list(ln.coords)
        for a, b in zip(cs, cs[1:]):
            i, j = node(a), node(b)
            if i != j:
                edges.append([i, j, round(math.dist(a, b), 1)])
    # Keep the largest connected piece (a stray stub can't be walked to).
    parent = list(range(len(nodes)))

    def find(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i
    for i, j, _ in edges:
        parent[find(i)] = find(j)
    sizes = {}
    for i in range(len(nodes)):
        sizes[find(i)] = sizes.get(find(i), 0) + 1
    big = max(sizes, key=sizes.get)
    keep = [i for i in range(len(nodes)) if find(i) == big]
    remap = {old: new for new, old in enumerate(keep)}
    nodes = [nodes[i] for i in keep]
    edges = [[remap[i], remap[j], d] for i, j, d in edges if i in remap and j in remap]
    lines = [LineString([nodes[i], nodes[j]]) for i, j, _ in edges]
    tree = shapely.STRtree(lines)

    def snap(x, y, max_off=120):
        """(edge, along from its first node, offset) for the nearest point on the network."""
        p = Point(x, y)
        k = tree.nearest(p)
        off = lines[k].distance(p)
        if off > max_off:
            return None
        return [int(k), round(lines[k].project(p), 1), round(off, 1)]

    # 2. The places already open.
    raw = overture_places(box)
    places, counts = [], {}
    for lng, lat, hier, conf, status in raw:
        if status != "open" or (conf or 0) < MIN_CONFIDENCE:
            continue
        k = kind_of(hier)
        if not k:
            continue
        x, y = to_local(lat, lng)
        if not reach.contains(Point(x, y)):
            continue
        s = snap(x, y)
        if s:
            places.append(s + [k])
            counts[k] = counts.get(k, 0) + 1

    # 3. What has opened since 2019: by kind and year, at the nearest corner.
    rows = survey.cached_json(LICENCES, {"$select": "naics,business_opened_date,latitude,longitude",
                                         "$where": f"within_box(geocoded_point, {box['n']}, {box['w']}, {box['s']}, {box['e']})",
                                         "$limit": 50000})
    node_tree = shapely.STRtree([Point(n) for n in nodes])
    opened, latest, by_sector = {}, "", {}
    for r in rows:
        sec = licence_sector(r.get("naics"))
        d = (r.get("business_opened_date") or "")[:10]
        if not sec or not d or d < "2019-01-01" or not r.get("latitude"):
            continue
        x, y = to_local(float(r["latitude"]), float(r["longitude"]))
        if not reach.contains(Point(x, y)):
            continue
        n = int(node_tree.nearest(Point(x, y)))
        slot = opened.setdefault(str(n), {})
        slot[sec] = slot.get(sec, 0) + 1
        by_sector[sec] = by_sector.get(sec, 0) + 1
        latest = max(latest, d)

    # 4. Rent, by tract.
    sw, ne = to_latlng(*league.bounds[:2]), to_latlng(*league.bounds[2:])
    tr = survey.cached_json(f"{TRACTS}/query", {
        "geometry": f"{min(sw[1], ne[1])},{min(sw[0], ne[0])},{max(sw[1], ne[1])},{max(sw[0], ne[0])}",
        "geometryType": "esriGeometryEnvelope", "inSR": 4326, "outSR": 4326, "spatialRel": "esriSpatialRelIntersects",
        "outFields": "GEOID,BASENAME", "f": "geojson"})
    tracts = []
    for f in tr["features"]:
        g = shape(f["geometry"])
        polys = [g] if g.geom_type == "Polygon" else list(g.geoms)
        local = [Polygon([to_local(lat, lng) for lng, lat in q.exterior.coords]) for q in polys]
        u = unary_union(local)
        if u.intersection(league).area < 2000:  # a sliver along the edge isn't a tract the area is in
            continue
        tracts.append({"geoid": f["properties"]["GEOID"], "name": f"Census tract {f['properties']['BASENAME']}",
                       "share_of_area": round(u.intersection(league).area / league.area, 3),
                       "rings": [[[round(x, 1), round(y, 1)] for x, y in q.simplify(2).exterior.coords] for q in survey.polys(u)]})
    acs = survey.cached_json(ACS, {"table_ids": "B25064,B25003,B25070", "geo_ids": ",".join("14000US" + t["geoid"] for t in tracts)})
    for t in tracts:
        d = acs["data"]["14000US" + t["geoid"]]
        e, m = d["B25064"]["estimate"], d["B25064"]["error"]
        ten, burden = d["B25003"]["estimate"], d["B25070"]["estimate"]
        computed = burden["B25070001"] - burden["B25070011"]
        t["median_rent"] = e["B25064001"]
        t["median_rent_moe"] = m["B25064001"]
        t["homes"] = ten["B25003001"]
        t["renting"] = round(ten["B25003003"] / ten["B25003001"], 3) if ten["B25003001"] else None
        t["rent_burdened"] = round(sum(burden[f"B25070{k:03d}"] for k in (7, 8, 9, 10)) / computed, 3) if computed else None
    release = acs["release"]

    # 5. How often a place is new: the metro's entry and exit rates, 2019-2023.
    sectors = {"shop": "44-45", "food": "72", "service": "81"}
    rates = {}
    rows = [r for r in csv.DictReader(io.StringIO(fetch_text(BDS))) if r["msa"] == CBSA and r["sector"] in sectors.values()
            and 2019 <= int(r["year"]) <= 2023]
    for sec, code in sectors.items():
        rs = [r for r in rows if r["sector"] == code]
        est = sum(int(r["estabs"]) for r in rs)
        rates[sec] = {"entry": round(sum(int(r["estabs_entry"]) for r in rs) / est, 4),
                      "exit": round(sum(int(r["estabs_exit"]) for r in rs) / est, 4),
                      "years": sorted(int(r["year"]) for r in rs), "naics": code}

    out = {
        "version": 1,
        "retrieved": datetime.date.today().isoformat(),
        "walk": {"speed_mps": WALK_MPS, "ring_m": WALK_RING_M, "note": "Along street centerlines at 3 miles an hour (ours); "
                 "crossings, sidewalks and alleys aren't modelled.", "skipped_classes": sorted(skipped)},
        "graph": {"nodes": nodes, "edges": edges},
        "kinds": [{"id": k, "label": label, "businesses": sorted(biz)} for k, label, _, biz in KINDS],
        "places": places,
        "places_by_kind": dict(sorted(counts.items())),
        "opened": {"by_node": opened, "by_sector": by_sector, "from": "2019-01-01", "to": latest},
        "tracts": tracts,
        "bds": rates,
        "sources": {
            "streets": "City of Norfolk, street centerlines (gisshare.norfolk.gov OpenData/Public_Works/FeatureServer/1)",
            "places": f"Overture Maps Foundation, places, release {OVERTURE_RELEASE} (CDLA Permissive 2.0): open, confidence ≥ {MIN_CONFIDENCE}",
            "opened": "City of Norfolk, Business Licenses (data.norfolk.gov dataset dpi6-sct5): new licences since 1 Jan 2019",
            "rent": f"U.S. Census Bureau, American Community Survey, {release['name']} (B25064, B25003, B25070), via Census Reporter",
            "bds": "U.S. Census Bureau, Business Dynamics Statistics, 2023 release (bds2023_msa_sec.csv), CBSA 47260",
        },
    }
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as f:
        json.dump(out, f, separators=(",", ":"))
    print(f"everyday.json: {len(nodes)} nodes, {len(edges)} edges, {len(places)} places {counts}, "
          f"{sum(by_sector.values())} licences {by_sector} to {latest}, {len(tracts)} tracts, BDS {rates}")


if __name__ == "__main__":
    main()
