"""
survey.py: the facts behind Ghent Streets' site, read from the City of Norfolk's public data.

The area is the Ghent Neighborhood League's civic-league polygon. Everything inside it (and a ring of
neighbours around it, drawn as context) comes from the city:

  League boundary    gisshare.norfolk.gov  OpenData/Neighborhoods_Group_OpenData/MapServer/0  (LEAGUE only;
                                           the officers' names and contacts are never requested)
  Zoning             gisshare.norfolk.gov  OpenData/Planning/FeatureServer/1        (ZONE)
  NFK2050 place type services2.arcgis.com  DRAFT_FutureLandUse2050/FeatureServer/3  (place_type; the City's
                                           ArcGIS Online layer, titled DRAFT, matching its printed Future Land
                                           Use Map of 10 Feb 2026)
  Parcels            gisshare.norfolk.gov  OpenData/Parcels/FeatureServer/1         (GPIN)
  Buildings          gisshare.norfolk.gov  InteractiveMapper/Property_Information_Group_IM/MapServer/1
                                           (FTR_CODE, the footprint's type; ELEV; DEMO)
  Street centerline  gisshare.norfolk.gov  OpenData/Public_Works/FeatureServer/1    (FULLNAME, ROW: the class,
                                           "arterial/collector" or "residential")
  Addresses          data.norfolk.gov      ere7-kake
  Assessment FY26    data.norfolk.gov      m5ya-5grb  (use, class, year built, lot and floor areas, assessed
                                           values; grantor, grantee and other owner fields are never requested)
  Trees              data.norfolk.gov      cmvv-agyb  (the City Tree Inventory: species, trunk diameter, place)

Facts only: what's eligible, what a ground floor would become and the tour are propose.py's.

Adapted from the Ghent Streets explorer's builder (jason-edelman.org, ghent-streets/data/scripts/
build_explorer.py, at 3efbc04), which read HC-G2 alone: the paged ArcGIS queries, corner lots, the
affine fit from state-plane feet to latitude/longitude, tree forms and the assessor's photos are its.

Geometry is requested in Virginia State Plane South (EPSG:2284, US feet) and written in metres from the
League area's centre, +x east, +y north.

Run:  tools/vibe-render/.venv/bin/python tools/ghent/survey.py [--photos]
Out:  data/ghent/survey.json   (raw responses are cached in target/ghent/cache, so a rerun is cheap on
      the city's servers; delete the cache to refetch)
Needs: shapely
"""

import hashlib
import json
import math
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import date

from shapely import STRtree
from shapely.geometry import LineString, MultiLineString, MultiPolygon, Point, Polygon, shape
from shapely.ops import unary_union

GIS = "https://gisshare.norfolk.gov/pubserver/rest/services"
LEAGUES = f"{GIS}/OpenData/Neighborhoods_Group_OpenData/MapServer/0"
ZONING = f"{GIS}/OpenData/Planning/FeatureServer/1"
PLACE_TYPES = "https://services2.arcgis.com/fUAsosS8WXl3mXcn/arcgis/rest/services/DRAFT_FutureLandUse2050/FeatureServer/3"
PARCELS = f"{GIS}/OpenData/Parcels/FeatureServer/1"
BUILDINGS = f"{GIS}/InteractiveMapper/Property_Information_Group_IM/MapServer/1"
STREETS = f"{GIS}/OpenData/Public_Works/FeatureServer/1"
SOCRATA = "https://data.norfolk.gov/resource"
PHOTOS = "https://gisshare.norfolk.gov/provalimages/001"
LEAGUE = "Ghent Neighborhood League"

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), "../.."))
OUT = os.path.join(ROOT, "data/ghent/survey.json")
CACHE = os.path.join(ROOT, "target/ghent/cache")
FT = 0.3048
UA = {"User-Agent": "street-smarts-ghent-survey/1.0 (+https://github.com/jedelman/street-smarts)"}
CONTEXT_RING_FT = 400          # neighbours drawn around the League area, as context
FRONTAGE_REACH_FT = 45         # centerline to lot line: half a 50-60 ft right-of-way, plus slack (the explorer's)
CORNER_MEET_FT = 120           # a corner lot's two streets meet within this of the lot (the explorer's)
# The building layer's own types (its FTR_CODE domain). Pavement, bridges and driveways aren't buildings.
BUILDING_TYPES = {2000: "residential", 2010: "commercial", 2020: "institutional", 2021: "school", 2022: "university",
                  2023: "hospital", 2024: "library", 2025: "recreation", 2026: "public safety", 2030: "garage",
                  2040: "rail station", 2050: "water tower", 2060: "storage tank", 2070: "other",
                  2080: "under construction"}
# The case studies built by hand in the explorer (and written up on the campaign site).
CASE_STUDIES = {"1428706223": "715-colonial", "1428629200": "740-w-princess-anne", "1428615872": "1012-colley",
                "1428701712": "801"}


def cached_json(url, params=None, tries=3):
    full = url + ("?" + urllib.parse.urlencode(params) if params else "")
    key = os.path.join(CACHE, hashlib.sha1(full.encode()).hexdigest() + ".json")
    if os.path.exists(key):
        with open(key) as f:
            return json.load(f)
    for i in range(tries):
        try:
            with urllib.request.urlopen(urllib.request.Request(full, headers=UA), timeout=120) as r:
                data = json.loads(r.read().decode("utf-8"))
            break
        except Exception:
            if i == tries - 1:
                raise
            time.sleep(2 * (i + 1))
    os.makedirs(CACHE, exist_ok=True)
    with open(key, "w") as f:
        json.dump(data, f)
    return data


def arcgis(layer, where="1=1", envelope=None, out_fields="*"):
    """All features of an ArcGIS layer (paged), as GeoJSON-like dicts in EPSG:2284 feet."""
    feats, offset = [], 0
    while True:
        p = {"where": where, "outFields": out_fields, "outSR": 2284, "f": "geojson",
             "resultOffset": offset, "resultRecordCount": 1000}
        if envelope:
            p.update({"geometry": ",".join(f"{v:.1f}" for v in envelope), "geometryType": "esriGeometryEnvelope",
                      "inSR": 2284, "spatialRel": "esriSpatialRelIntersects"})
        d = cached_json(f"{layer}/query", p)
        page = d.get("features", [])
        feats += page
        if len(page) < 1000 and not d.get("exceededTransferLimit") and not d.get("properties", {}).get("exceededTransferLimit"):
            return feats
        offset += len(page)


def socrata(dataset, field, values, select):
    rows, vals = [], sorted(set(values))
    for i in range(0, len(vals), 150):
        chunk = ",".join(f"'{v}'" for v in vals[i:i + 150])
        rows += cached_json(f"{SOCRATA}/{dataset}.json", {"$select": select, "$where": f"{field} in({chunk})", "$limit": 5000})
    return rows


def geom(f):
    g = f.get("geometry")
    try:
        return shape(g) if g else None
    except Exception:
        return None


def polys(g):
    return [g] if isinstance(g, Polygon) else list(g.geoms) if isinstance(g, MultiPolygon) else []


def lines(g):
    return [g] if isinstance(g, LineString) else list(g.geoms) if isinstance(g, MultiLineString) else []


def tree_form(name, genus):
    """A drawing form for a tree, from its common name or genus (the explorer's)."""
    n = f"{name or ''} {genus or ''}".lower()
    for form, words in (
        ("crape", ("crape", "lagerstroemia")),
        ("plane", ("sycamore", "planetree", "plane tree", "platanus")),
        ("pine", ("pine", "pinus")),
        ("cone", ("cedar", "juniper", "cypress", "holly", "ilex", "spruce", "arborvitae")),
        ("evergreen", ("magnolia", "live oak", "virginiana")),
        ("small", ("dogwood", "redbud", "cherry", "plum", "crabapple", "pear", "hawthorn", "myrtle", "serviceberry", "vitex")),
        ("oak", ("oak", "quercus", "elm", "ulmus", "ginkgo", "tulip", "liriodendron", "hackberry", "zelkova", "linden")),
        ("maple", ("maple", "acer", "ash", "fraxinus", "birch", "sweetgum", "liquidambar", "gum")),
    ):
        if any(w in n for w in words):
            return form
    return "round"


def stories_of(cls):
    """Stories from the assessor's class description, where it says ("=2 but <3 Sty" -> 2, ">1 but <2" -> 1.5,
    "=3 and higher" -> 3), else None."""
    if not cls:
        return None
    if re.search(r"=\s*3 and higher", cls):
        return 3.0
    if re.search(r">\s*1 but <\s*2", cls):
        return 1.5
    m = re.search(r"=\s*(\d+(?:\.\d+)?)\s*but", cls) or re.search(r"(\d+(?:\.\d+)?)\s*Sty", cls)
    return float(m.group(1)) if m else None


def affine_fit(pairs):
    """Least-squares lat = a·x + b·y + c and lng = d·x + e·y + f from (x, y, lat, lng) samples (the explorer's)."""
    n = len(pairs)
    mx, my = sum(p[0] for p in pairs) / n, sum(p[1] for p in pairs) / n
    sxx = sum((p[0] - mx) ** 2 for p in pairs)
    syy = sum((p[1] - my) ** 2 for p in pairs)
    sxy = sum((p[0] - mx) * (p[1] - my) for p in pairs)
    det = sxx * syy - sxy * sxy

    def solve(k):
        mv = sum(p[k] for p in pairs) / n
        sxv = sum((p[0] - mx) * (p[k] - mv) for p in pairs)
        syv = sum((p[1] - my) * (p[k] - mv) for p in pairs)
        a = (sxv * syy - syv * sxy) / det
        b = (syv * sxx - sxv * sxy) / det
        return a, b, mv - a * mx - b * my
    return solve(2), solve(3)


def head(url, tries=3):
    for i in range(tries):
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers=UA, method="HEAD"), timeout=30) as r:
                return r.headers.get("Last-Modified") or ""
        except urllib.error.HTTPError:
            return None
        except Exception:
            time.sleep(1 + i)
    return None


def assessor_photo(pid):
    """The newest of the assessor's street photos of a parcel, on the city's own server (the explorer's).
    Filed by parcel ID: 001/<first four digits>/<last four><R|C><card><n>.jpg."""
    from email.utils import parsedate_to_datetime
    found = []
    for card in ("R01", "C01", "R02", "C02", "C00", "R00"):
        for n in range(1, 10):
            url = f"{PHOTOS}/{pid[:4]}/{pid[4:8]}{card}{n}.jpg"
            modified = head(url)
            if modified is not None:
                found.append((modified, n, url))
            elif n >= 2:
                break
            time.sleep(0.1)
    if not found:
        return None

    def key(f):
        try:
            return (parsedate_to_datetime(f[0]).timestamp(), f[1])
        except Exception:
            return (0, f[1])
    return max(found, key=key)[2]


def main(photos_for=None):
    # 1. The area: the League's polygon, and a ring of neighbours around it.
    league = unary_union([geom(f) for f in arcgis(LEAGUES, f"LEAGUE='{LEAGUE}'", out_fields="LEAGUE") if geom(f)])
    minx, miny, maxx, maxy = league.bounds
    env = (minx - CONTEXT_RING_FT, miny - CONTEXT_RING_FT, maxx + CONTEXT_RING_FT, maxy + CONTEXT_RING_FT)
    cx, cy = (minx + maxx) / 2, (miny + maxy) / 2
    near = league.buffer(CONTEXT_RING_FT)

    def local(x, y):
        return [round((x - cx) * FT, 2), round((y - cy) * FT, 2)]

    def ring(poly, tol=0.5):
        pts = []
        for x, y in list(poly.simplify(tol).exterior.coords)[:-1]:
            q = local(x, y)
            if not pts or q != pts[-1]:
                pts.append(q)
        if len(pts) > 1 and pts[0] == pts[-1]:
            pts.pop()
        return pts

    # 2. Zoning and NFK2050 place types, as polygons to look parcels up in.
    zones = [(f["properties"].get("ZONE"), geom(f)) for f in arcgis(ZONING, envelope=env, out_fields="ZONE") if geom(f)]
    places = [(f["properties"].get("place_type"), geom(f)) for f in arcgis(PLACE_TYPES, envelope=env, out_fields="place_type") if geom(f)]
    zone_tree, place_tree = STRtree([g for _, g in zones]), STRtree([g for _, g in places])

    def lookup(pt, tree, items):
        for i in tree.query(pt):
            if items[i][1].contains(pt):
                return items[i][0]
        return None

    # 3. Streets: centerlines with names and their class.
    streets = []
    for f in arcgis(STREETS, envelope=env, out_fields="FULLNAME,ROW"):
        g, name = geom(f), (f["properties"].get("FULLNAME") or "").strip()
        if g and name:
            streets.append({"name": name, "row": f["properties"].get("ROW"), "geom": g})
    merged, classes = {}, {}
    for s in streets:
        merged.setdefault(s["name"], []).append(s["geom"])
        classes.setdefault(s["name"], set()).add(s["row"])
    merged = {n: unary_union(gs) for n, gs in merged.items()}
    seg_tree = STRtree([s["geom"] for s in streets])

    # 4. Parcels whose interior point lies in the League area: the site. Each with its zone, place type,
    #    the streets it fronts (and their class where it fronts them), and whether it's a corner lot.
    parcels = []
    for f in arcgis(PARCELS, envelope=env, out_fields="GPIN"):
        g, gpin = geom(f), (f["properties"].get("GPIN") or "").strip()
        if not g or not gpin or g.area < 50:
            continue
        pt = g.representative_point()
        if not league.contains(pt):
            continue
        front = {}
        for i in seg_tree.query(g.buffer(FRONTAGE_REACH_FT)):
            s = streets[i]
            d = g.distance(s["geom"])
            if d <= FRONTAGE_REACH_FT and (s["name"] not in front or d < front[s["name"]]["d"]):
                front[s["name"]] = {"d": d, "row": s["row"]}
        names = sorted(front, key=lambda n: front[n]["d"])
        corner = any(not merged[a].intersection(merged[b]).is_empty and g.distance(merged[a].intersection(merged[b])) <= CORNER_MEET_FT
                     for i, a in enumerate(names) for b in names[i + 1:])
        parcels.append({"gpin": gpin, "geom": g, "zone": lookup(pt, zone_tree, zones), "place": lookup(pt, place_tree, places),
                        "streets": [[n, front[n]["row"]] for n in names], "corner": corner})
    by_gpin = {p["gpin"]: p for p in parcels}
    parcel_tree = STRtree([p["geom"] for p in parcels])

    # 5. Buildings in the area and its ring, each with the parcel holding its interior point. A row's
    #    footprint is often one polygon across several lots; where a site parcel takes a real piece of a
    #    footprint it doesn't own, that piece is cut out and given to it, so every lot has its own house.
    buildings = []
    for f in arcgis(BUILDINGS, envelope=env, out_fields="ELEV,FTR_CODE,DEMO"):
        g, pr = geom(f), f["properties"]
        code = pr.get("FTR_CODE")
        if not g or pr.get("DEMO") == 1 or code not in BUILDING_TYPES or not near.intersects(g):
            continue
        g = g.buffer(0)
        hits = [parcels[i] for i in parcel_tree.query(g) if parcels[i]["geom"].intersects(g)]
        pieces = []
        if len(hits) > 1:
            rest = g
            for p in hits:
                part = g.intersection(p["geom"])
                if part.area > 200:  # ft², about 19 m²: a real share of a row, not a sliver over the lot line
                    pieces.append((part, p))
                    rest = rest.difference(p["geom"])
            if rest.area > 200:
                pieces.append((rest, None))
        if not pieces:
            owner = next((p for p in hits if p["geom"].contains(g.representative_point())), None)
            pieces = [(g, owner)]
        for piece, who in pieces:
            for poly in (q for part in getattr(piece, "geoms", [piece]) for q in polys(part)):
                if poly.area < 30:
                    continue
                elev = pr.get("ELEV")
                # ELEV is labelled only "Elevation"; read as the roof's elevation in feet, less ~7 ft of ground
                # (the explorer's reading). Where it's missing, propose.py infers the height.
                h = round(min(max(elev - 7, 9), 220) * FT, 1) if elev and elev > 12 else None
                buildings.append({"parcel": who["gpin"] if who else None, "type": BUILDING_TYPES[code], "h": h,
                                  "in_area": bool(league.intersects(poly)), "poly": ring(poly)})

    # 6. Addresses and assessments, by GPIN (owners deliberately not requested).
    gpins = list(by_gpin)
    addr, pairs = {}, []
    for r in socrata("ere7-kake", "gpin", gpins, "gpin,full_address,parcel_centroid_latitude,parcel_centroid_longitude,"
                     "state_plane_x_coordinate,state_plane_y_coordinate"):
        a = (r.get("full_address") or "").strip()
        if a and a.upper() != "NO ADDRESS":
            addr.setdefault(r["gpin"], set()).add(a)
        try:
            pairs.append((float(r["state_plane_x_coordinate"]), float(r["state_plane_y_coordinate"]),
                          float(r["parcel_centroid_latitude"]), float(r["parcel_centroid_longitude"])))
        except (KeyError, TypeError, ValueError):
            pass
    la, lo = affine_fit(pairs)
    to_ll = lambda x, y: (la[0] * x + la[1] * y + la[2], lo[0] * x + lo[1] * y + lo[2])
    det2 = la[0] * lo[1] - la[1] * lo[0]
    from_ll = lambda lat, lng: (((lat - la[2]) * lo[1] - (lng - lo[2]) * la[1]) / det2,
                                (la[0] * (lng - lo[2]) - lo[0] * (lat - la[2])) / det2)
    worst = max(math.hypot((to_ll(x, y)[0] - a) * 111000, (to_ll(x, y)[1] - b) * 89000) for x, y, a, b in pairs)
    print(f"coordinate fit: {len(pairs)} address points, worst residual {worst:.2f} m")
    assess = {}
    for r in socrata("m5ya-5grb", "gpin", gpins,
                     "gpin,parcel_id,extension,property_class_description,property_use,improvement_year_built,"
                     "land_square_footage,residential_finished_living,commercial_building_area,"
                     "current_land_value,current_improvement_value"):
        assess.setdefault(r["gpin"], []).append(r)

    def num(v):
        try:
            return int(float(v))
        except (TypeError, ValueError):
            return None

    # 7. A Street View viewpoint for each parcel (the nearest point on its nearest street, looking at it).
    for p in parcels:
        p["sv"] = None
        if p["streets"]:
            line = merged[p["streets"][0][0]]
            c = p["geom"].representative_point()
            on = line.interpolate(line.project(c))
            lat, lng = to_ll(on.x, on.y)
            p["sv"] = [round(lat, 6), round(lng, 6), round((math.degrees(math.atan2(c.x - on.x, c.y - on.y)) + 360) % 360)]

    # 8. The assessor's street photos, where asked (propose.py names the parcels the walk shows them for).
    photos = {}
    photo_file = os.path.join(ROOT, "data/ghent/photos.json")
    if os.path.exists(photo_file):
        with open(photo_file) as f:
            photos = json.load(f)
    if photos_for:
        for gpin in photos_for:
            rows = assess.get(gpin) or []
            pid = rows[0].get("parcel_id") if rows else None
            if gpin in photos or not pid or len(pid) != 8 or not pid.isdigit():
                continue
            photos[gpin] = assessor_photo(pid)
        os.makedirs(os.path.dirname(photo_file), exist_ok=True)
        with open(photo_file, "w") as f:
            json.dump(photos, f, indent=0, sort_keys=True)

    # 9. Trees: the city's inventory, mostly street and public trees. Diameter is the trunk at breast height
    #    in inches: a measurement where there is one, else the middle of its range, else 0 (unknown).
    corners_ll = [to_ll(x, y) for x, y in ((env[0], env[1]), (env[2], env[3]), (env[0], env[3]), (env[2], env[1]))]
    lat0, lat1 = min(c[0] for c in corners_ll), max(c[0] for c in corners_ll)
    lng0, lng1 = min(c[1] for c in corners_ll), max(c[1] for c in corners_ll)
    inventory = cached_json(f"{SOCRATA}/cmvv-agyb.json", {
        "$select": "common_name,genus,trunk_diameter,trunk_diameter_range,latitude,longitude",
        "$where": f"within_box(latitude_longitude_point,{lat1},{lng0},{lat0},{lng1})", "$limit": 50000})
    ranges = {'0" to 6"': 3, '6" to 12"': 9, '12" to 18"': 15, '18" to 24"': 21, '24" to 30"': 27,
              '30" to 36"': 33, '36" to 42"': 39, '42" to 48"': 45, '48" +': 50}
    trees = []
    for t in inventory:
        try:
            x, y = from_ll(float(t["latitude"]), float(t["longitude"]))
        except (KeyError, TypeError, ValueError):
            continue
        if not near.contains(Point(x, y)):
            continue
        try:
            dbh = float(t.get("trunk_diameter")) if t.get("trunk_diameter") else ranges.get(t.get("trunk_diameter_range"))
        except ValueError:
            dbh = ranges.get(t.get("trunk_diameter_range"))
        trees.append([*local(x, y), tree_form(t.get("common_name"), t.get("genus")), round(dbh or 0, 1),
                      (t.get("common_name") or "").strip()])

    out_parcels = []
    for p in parcels:
        rows = assess.get(p["gpin"]) or [{}]
        a = rows[0]
        cls = a.get("property_class_description")
        poly = max(polys(p["geom"]), key=lambda q: q.area)
        out_parcels.append({
            "gpin": p["gpin"], "pid": a.get("parcel_id"),
            "address": sorted(addr.get(p["gpin"], []))[0] if addr.get(p["gpin"]) else None,
            "addresses": len(addr.get(p["gpin"], [])),
            "use": a.get("property_use"), "cls": cls, "stories": stories_of(cls), "cards": len(rows),
            "built": num(a.get("improvement_year_built")), "lot_sqft": num(a.get("land_square_footage")),
            "living_sqft": num(a.get("residential_finished_living")), "commercial_sqft": num(a.get("commercial_building_area")),
            "land_value": sum(num(r.get("current_land_value")) or 0 for r in rows),
            "improvement_value": sum(num(r.get("current_improvement_value")) or 0 for r in rows),
            "zone": p["zone"], "place": p["place"], "corner": p["corner"], "streets": p["streets"][:4],
            "case": CASE_STUDIES.get(p["gpin"]), "photo": photos.get(p["gpin"]), "sv": p["sv"],
            "poly": ring(poly),
        })

    clip = Polygon([(env[0], env[1]), (env[2], env[1]), (env[2], env[3]), (env[0], env[3])])
    zone_g2 = unary_union([g for z, g in zones if z == "HC-G2"]).intersection(clip)
    out = {
        "version": 1,
        "meta": {
            "built": date.today().isoformat(),
            "area": f"{LEAGUE} (the City's civic-league polygon), with a {CONTEXT_RING_FT} ft ring of neighbours as context",
            "crs": "metres east/north of the League area's centre; source EPSG:2284 (VA State Plane South, US ft)",
            "centre_ft": [round(cx, 1), round(cy, 1)],
            "to_latlng": {"lat": [la[0] / FT, la[1] / FT, la[0] * cx + la[1] * cy + la[2]],
                          "lng": [lo[0] / FT, lo[1] / FT, lo[0] * cx + lo[1] * cy + lo[2]],
                          "note": "lat = a·x + b·y + c with x, y in this file's metres (an affine fit to the City's address points)"},
            "sources": {"league": LEAGUES, "zoning": ZONING, "place_types": PLACE_TYPES, "parcels": PARCELS,
                        "buildings": BUILDINGS, "streets": STREETS, "addresses": "https://data.norfolk.gov/d/ere7-kake",
                        "assessment": "https://data.norfolk.gov/d/m5ya-5grb", "trees": "https://data.norfolk.gov/d/cmvv-agyb",
                        "photos": "City of Norfolk Real Estate Assessor (gisshare.norfolk.gov)"},
            "heights": "approximate: the City's building 'Elevation' field read as roof elevation in feet, less ~7 ft of ground; "
                       "null where the field is missing (propose.py infers it)",
        },
        "league": [ring(q, 1) for q in polys(league)],
        "hcg2": [ring(q, 1) for q in polys(zone_g2)],
        "parcels": out_parcels,
        "buildings": buildings,
        "streets": [{"name": s["name"], "row": s["row"], "line": [local(x, y) for x, y in seg.coords]}
                    for s in streets for seg in lines(s["geom"].intersection(clip)) if len(seg.coords) > 1],
        "trees": trees,
    }
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(out, fh, separators=(",", ":"))
    print(f"{len(out_parcels)} parcels ({sum(p['corner'] for p in out_parcels)} corner), {len(buildings)} buildings "
          f"({sum(b['in_area'] for b in buildings)} in the area), {len(trees)} trees, {len(out['streets'])} street pieces "
          f"-> {os.path.relpath(OUT, ROOT)} ({os.path.getsize(OUT) // 1024} KB)")


if __name__ == "__main__":
    want = None
    if "--photos" in sys.argv:
        with open(os.path.join(ROOT, "target/ghent/photo_wanted.json")) as f:
            want = json.load(f)
    main(want)
