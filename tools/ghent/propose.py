"""
propose.py: Ghent Streets' proposal, made from the survey's facts. Everything here is a choice, and says whose.

1. Which parcels convert: Jason's maximal implementation (2026-11-27), "every parcel that would be allowed to
   convert to retail use under the NFK2050 master plan rules in the Ghent Neighborhood League area, plus the corner
   parcel additions we suggested, plus every parcel with frontage on Graydon and Blow." Read as RULES below, each
   with the plan's words and page.
2. What each ground floor becomes: one of the uses Policy Brief No. 1 would permit by right, drawn in Policy Brief
   No. 2's stated mix (50% food and beverage, 30% retail goods, 20% personal and professional services); the split
   within each is ours. Bars are left out (Brief 1 keeps them conditional). One unit per parcel, at most 3,500 sq ft
   (Brief 1's by-right size limit).
3. How it meets the street: the Architectural Review Board's guidelines, as the explorer read them, and Jason's
   call for this pass: "keep doors and windows where they are. It's an adaptive-reuse project, we're adapting the
   existing building, not remodeling it." A commercial building keeps its storefront; every other building keeps
   its doors and windows, and the new use shows through them.
4. The tour: six converted properties as different from each other as possible, whose view sheds don't overlap.
5. The waypoint list: the corner lots the campaign identified (HC-G2's, by the explorer's rule).

Run:  tools/vibe-render/.venv/bin/python tools/ghent/propose.py
In:   data/ghent/survey.json      Out: data/ghent/proposal.json, target/ghent/photo_wanted.json
Needs: shapely, numpy
"""

import hashlib
import itertools
import json
import math
import os

import numpy as np
from shapely import STRtree
from shapely.geometry import LineString, Point, Polygon
from shapely.ops import unary_union

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), "../.."))
SURVEY = os.path.join(ROOT, "data/ghent/survey.json")
OUT = os.path.join(ROOT, "data/ghent/proposal.json")
SQFT = 10.7639

ARTERIAL = "arterial/collector"
SUSTAIN, INNOVATION, ACTIVITY = "Sustain - Traditional", "Innovation and Production", "Regional Activity Center"
NFK2050 = "NFK2050 Comprehensive Plan, adopted December 9, 2025 (Ordinance No. 50201)"
RULES = {
    "nfk2050-arterial": {
        "label": "On an arterial or collector street, in a Sustain place type",
        "source": NFK2050, "page": 69,
        "quote": "The Secondary, non-residential uses envisioned for the Sustain areas are appropriate primarily along "
                 "arterials and collector streets, preserving the residential character of the neighborhood interiors.",
        "how": "The parcel lies in NFK2050's Sustain – Traditional place type and comes within 45 ft of a street the "
               "City's centerline layer classes as an arterial or collector.",
    },
    "nfk2050-innovation": {
        "label": "In NFK2050's Innovation and Production place type (the medical campus)",
        "source": NFK2050, "page": 81,
        "quote": "Residential uses may be included in mixed-use or live/work configurations, but ground floors should "
                 "be accessible and active from the street.",
        "how": "The plan lists commercial/retail as a secondary use here and asks for supporting amenities, food and "
               "retail among them (p. 80).",
    },
    "nfk2050-activity": {
        "label": "In NFK2050's Regional Activity Center place type",
        "source": NFK2050, "page": 82,
        "quote": "Regional Activity Centers integrate residential, commercial, and cultural uses, creating a lively and "
                 "dynamic urban environment.",
        "how": "The plan's chart lists commercial/retail among the place type's uses.",
    },
    "corner": {
        "label": "A corner lot in HC-G2 (the campaign's addition)",
        "source": "Ghent Streets' parcel explorer: the lot's edge comes within 45 ft of two differently named streets "
                  "that meet within 120 ft of it",
        "quote": "Small-scale retail or other complementary community-facing uses are encouraged at key nodes or "
                 "intersections, with consistent street frontage.",
        "page": 93,
        "how": "The campaign reads NFK2050's \"key nodes or intersections\" (p. 93) as HC-G2's corners.",
    },
    "graydon-blow": {
        "label": "Fronting Graydon Avenue or Blow Street (Jason's addition)",
        "source": "Jason Edelman, 2026-11-27", "page": None, "quote": None,
        "how": "The parcel comes within 45 ft of Graydon Avenue's or Blow Street's centerline. Both are classed residential.",
    },
}
HOUSE = ("Single Family", "Duplex", "Triplex", "Fourplex")
# Brief 2's mix (an assumption it states as such), and our split within it, over businesses the walk draws
# (public/sparrow/shopfronts.js). Every one is a use Brief 1 would permit by right.
MIX = {"food": 0.50, "retail": 0.30, "services": 0.20}
WITHIN = {
    "food": {"coffee": 0.6, "bistro": 0.4},
    "retail": {"books": 0.2, "greengrocer": 0.2, "bakery": 0.15, "florist": 0.15, "hardware": 0.1, "bikes": 0.1, "records": 0.1},
    "services": {"hairdresser": 0.35, "accountant": 0.2, "architect": 0.15, "design": 0.15, "tailor": 0.15},
}
USE_OF = {"food": "a small restaurant or café", "retail": "neighborhood retail", "services": "a personal or professional service"}
BY_RIGHT_MAX_SQFT = 3500
# The case studies keep the first use their own pages show (ghent-streets/case-studies/*.html).
CASE_USES = {"715-colonial": "bistro", "740-w-princess-anne": "coffee", "1012-colley": "books"}


def rng01(*key):
    return int(hashlib.sha1(":".join(map(str, key)).encode()).hexdigest()[:12], 16) / 16 ** 12


def poly_area(pts):
    return 0.5 * sum(pts[i][0] * pts[(i + 1) % len(pts)][1] - pts[(i + 1) % len(pts)][0] * pts[i][1] for i in range(len(pts)))


def frontage(b, street_lines):
    """The building's face to the street (the explorer's frontage()): edges at least 3 m long facing the nearest
    street within about 60°, and among those within 2.5 m of the nearest, the longest."""
    pts = b["poly"]
    cw = poly_area(pts) < 0
    edges = []
    for i in range(len(pts)):
        a, c = pts[i], pts[(i + 1) % len(pts)]
        L = math.hypot(c[0] - a[0], c[1] - a[1])
        if L < 3:
            continue
        d = ((c[0] - a[0]) / L, (c[1] - a[1]) / L)
        n = (-d[1], d[0]) if cw else (d[1], -d[0])
        mid = ((a[0] + c[0]) / 2, (a[1] + c[1]) / 2)
        best = None
        for ln in street_lines:
            q = ln.interpolate(ln.project(Point(mid)))
            dist = math.hypot(q.x - mid[0], q.y - mid[1])
            if best is None or dist < best[0]:
                best = (dist, (q.x, q.y))
        if not best or best[0] > 60:
            continue
        toward = ((best[1][0] - mid[0]) * n[0] + (best[1][1] - mid[1]) * n[1]) / (best[0] or 1)
        if toward < 0.5:
            continue
        edges.append({"i": i, "a": a, "c": c, "d": d, "n": n, "len": L, "street": best[0], "q": best[1]})
    if not edges:
        return None
    nearest = min(e["street"] for e in edges)
    return max((e for e in edges if e["street"] <= nearest + 2.5), key=lambda e: e["len"])


def viewshed(stand, blockers, radius=70.0, rays=240):
    """What can be seen from `stand` at street level: a polygon of the ends of `rays` sight lines, each stopped at
    the first building it meets, out to `radius` metres."""
    x0, y0 = stand
    near = [g for g in blockers.query(Point(stand).buffer(radius))]
    geoms = [blockers.geometries[i] for i in near]
    pts = []
    for k in range(rays):
        t = 2 * math.pi * k / rays
        end = (x0 + radius * math.cos(t), y0 + radius * math.sin(t))
        ray = LineString([stand, end])
        best = radius
        for g in geoms:
            if not g.intersects(ray):
                continue
            hit = g.boundary.intersection(ray)
            for p in getattr(hit, "geoms", [hit]):
                if p.is_empty:
                    continue
                for xy in getattr(p, "coords", []):
                    dd = math.hypot(xy[0] - x0, xy[1] - y0)
                    if 0.05 < dd < best:
                        best = dd
        pts.append((x0 + best * math.cos(t), y0 + best * math.sin(t)))
    return Polygon(pts).buffer(0)


def main():
    S = json.load(open(SURVEY))
    parcels = S["parcels"]
    by_gpin = {p["gpin"]: p for p in parcels}
    lines_by_name = {}
    for s in S["streets"]:
        lines_by_name.setdefault(s["name"], []).append(LineString(s["line"]))
    blds = {}
    for i, b in enumerate(S["buildings"]):
        if b["parcel"]:
            blds.setdefault(b["parcel"], []).append(i)

    # 1. Eligibility: each rule a parcel meets.
    out = {}
    for p in parcels:
        reasons = []
        classes = {row for _, row in p["streets"]}
        names = {n for n, _ in p["streets"]}
        if p["place"] == SUSTAIN and ARTERIAL in classes:
            reasons.append("nfk2050-arterial")
        if p["place"] == INNOVATION:
            reasons.append("nfk2050-innovation")
        if p["place"] == ACTIVITY:
            reasons.append("nfk2050-activity")
        if p["corner"] and p["zone"] == "HC-G2":
            reasons.append("corner")
        if names & {"GRAYDON AVENUE", "BLOW STREET"}:
            reasons.append("graydon-blow")
        mine = [S["buildings"][i] for i in blds.get(p["gpin"], []) if S["buildings"][i]["type"] not in ("garage",)]
        main_b = max(mine, key=lambda b: abs(poly_area(b["poly"])), default=None)
        if not reasons:
            status = "not-eligible"
        elif p["use"] == "Commercial" or (main_b and main_b["type"] == "commercial" and p["use"] in (None, "Non-Taxable")):
            status = "existing"
        elif not main_b:
            status = "nothing-to-adapt"   # vacant land, a parking lot, a park: no building to reuse
        else:
            status = "converted"
        rec = {"status": status, "reasons": reasons, "amendment": p["zone"] == "HC-G2"}
        if main_b:
            rec["building"] = S["buildings"].index(main_b)
        out[p["gpin"]] = rec

    # 2. What each converted ground floor becomes, and how it meets the street.
    converted = [g for g, r in out.items() if r["status"] == "converted"]
    placed = []  # (x, y, business) for the no-repeats-next-door rule
    for gpin in sorted(converted, key=lambda g: rng01(g, "order")):
        p, r = by_gpin[gpin], out[gpin]
        b = S["buildings"][r["building"]]
        street_lines = [ln for n, _ in p["streets"][:1] for ln in lines_by_name.get(n, [])]
        f = frontage(b, street_lines) or frontage(b, [ln for n, _ in p["streets"] for ln in lines_by_name.get(n, [])])
        if not f:
            r["status"] = "nothing-to-adapt"
            r["why"] = "no wall of its building faces a street"
            continue
        house = p["use"] and p["use"].startswith(HOUSE)
        if b["type"] == "commercial":
            typ = "storefront"
        elif house:
            typ = "house"
        elif b["type"] in ("residential", "other") or (p["use"] or "").startswith(("Apartment", "Condo", "Co-op")):
            typ = "flats"
        else:
            typ = "institution"
        cx, cy = np.mean(b["poly"], axis=0)
        if p["case"] in CASE_USES:
            bz = CASE_USES[p["case"]]
            k = next(g for g, split in WITHIN.items() if bz in split)
        else:
            # The group by Brief 2's mix; then, within it, a business no neighbour within 45 m already has, if any.
            t = rng01(gpin, "group")
            for k, share in MIX.items():
                t -= share
                if t <= 0:
                    break
            recent = {z for (x, y, z) in placed if math.hypot(x - cx, y - cy) < 45}
            pool = [(z, w) for z, w in WITHIN[k].items() if z not in recent] or list(WITHIN[k].items())
            t = rng01(gpin, "business") * sum(w for _, w in pool)
            for bz, w in pool:
                t -= w
                if t <= 0:
                    break
        placed.append((cx, cy, bz))
        foot_sqft = abs(poly_area(b["poly"])) * SQFT
        r.update({"typology": typ, "business": bz, "kind": k, "use": USE_OF[k],
                  "unit_sqft": int(min(foot_sqft, BY_RIGHT_MAX_SQFT)), "footprint_sqft": int(foot_sqft),
                  "front": {"edge": f["i"], "a": f["a"], "c": f["c"], "n": [round(f["n"][0], 4), round(f["n"][1], 4)],
                            "street": round(f["street"], 1), "toward": [round(f["q"][0], 2), round(f["q"][1], 2)]}})

    # 3. The tour: six converted properties, as different as possible, with view sheds that don't overlap.
    blockers = STRtree([Polygon(b["poly"]).buffer(0) for b in S["buildings"] if len(b["poly"]) >= 3])
    trunks = STRtree([Point(t[0], t[1]).buffer(0.6 + (t[3] or 7) * 0.0254 / 2) for t in S["trees"]])

    def clear_stand(stand, look, along):
        """Slide a stand along the facade (up to 6 m either way) until the view from just behind it to the door
        passes no tree trunk and no building."""
        for off in (0, 2, -2, 4, -4, 6, -6):
            sx, sy = stand[0] + along[0] * off, stand[1] + along[1] * off
            dx, dy = look[0] - sx, look[1] - sy
            L = math.hypot(dx, dy) or 1
            back = (sx - dx / L * 4.5, sy - dy / L * 4.5)  # the camera sits behind the sparrow
            line = LineString([back, (look[0] - dx / L * 1.5, look[1] - dy / L * 1.5)])
            if not trunks.query(line, predicate="intersects").size and not blockers.query(line, predicate="crosses").size:
                return (sx, sy)
        return stand
    feats, cands = [], []
    eras = lambda y: 0 if not y else 1 if y < 1900 else 2 if y < 1920 else 3 if y < 1950 else 4
    for gpin, r in out.items():
        if r["status"] != "converted":
            continue
        p, b = by_gpin[gpin], S["buildings"][r["building"]]
        f = r["front"]
        # Stand across the street from the door: on the far side of the centerline, facing the front.
        mx, my = (f["a"][0] + f["c"][0]) / 2, (f["a"][1] + f["c"][1]) / 2
        n = f["n"]
        stand = (mx + n[0] * (f["street"] * 2 if f["street"] < 14 else f["street"] + 6), my + n[1] * (f["street"] * 2 if f["street"] < 14 else f["street"] + 6))
        stand = clear_stand(stand, (mx, my), (f["a"][0] - f["c"][0], f["a"][1] - f["c"][1]) and
                            ((f["c"][0] - f["a"][0]) / (math.dist(f["a"], f["c"]) or 1), (f["c"][1] - f["a"][1]) / (math.dist(f["a"], f["c"]) or 1)))
        cands.append({"gpin": gpin, "stand": stand, "look": (mx, my)})
        feats.append([
            ["house", "flats", "storefront", "institution"].index(r["typology"]),
            ["food", "retail", "services"].index(r["kind"]),
            eras(p["built"]),
            min(4, (p["stories"] or (b["h"] or 7) / 3.2)),
            math.log10(max(200, p["lot_sqft"] or 2000)),
            sum(1 << i for i, k in enumerate(RULES) if k in r["reasons"]),
            1 if r["amendment"] else 0,
            sorted(b2 for split in WITHIN.values() for b2 in split).index(r["business"]),
        ])
    F = np.array(feats, float)
    # Gower-style: categorical columns match or don't; numeric columns scaled by their range.
    cat = [0, 1, 2, 5, 6, 7]
    num = [3, 4]
    rng = np.ptp(F[:, num], axis=0)
    rng[rng == 0] = 1

    def dist(i, j):
        c = np.mean([F[i, k] != F[j, k] for k in cat])
        n_ = np.mean(np.abs(F[i, num] - F[j, num]) / rng)
        return (len(cat) * c + len(num) * n_) / (len(cat) + len(num))

    sheds = {}

    def shed(i):
        if i not in sheds:
            sheds[i] = viewshed(cands[i]["stand"], blockers)
        return sheds[i]

    # Start from the case study with the most rules behind it, if it converts; else the candidate farthest from the rest.
    order = list(range(len(cands)))
    case_idx = [i for i in order if by_gpin[cands[i]["gpin"]]["case"]]
    first = max(case_idx, key=lambda i: (len(out[cands[i]["gpin"]]["reasons"]), cands[i]["gpin"])) if case_idx else 0
    chosen = [first]
    while len(chosen) < 6:
        best, best_d = None, -1
        for i in sorted(order, key=lambda i: -min(dist(i, j) for j in chosen)):
            dmin = min(dist(i, j) for j in chosen)
            if dmin <= best_d:
                break
            if all(shed(i).intersection(shed(j)).area < 1.0 for j in chosen):
                best, best_d = i, dmin
                break
        if best is None:
            break
        chosen.append(best)
    # The shortest walk through them, starting at the first.
    def length(seq):
        return sum(math.dist(cands[a]["stand"], cands[b]["stand"]) for a, b in zip(seq, seq[1:]))
    rest = chosen[1:]
    route = [chosen[0], *min(itertools.permutations(rest), key=lambda s: length([chosen[0], *s]))]
    waypoints = []
    for i in route:
        c, gpin = cands[i], cands[i]["gpin"]
        waypoints.append({"id": f"tour-{gpin}", "gpin": gpin, "stand": [round(c["stand"][0], 2), round(c["stand"][1], 2)],
                          "look": [round(c["look"][0], 2), round(c["look"][1], 2)],
                          "shed_m2": round(shed(i).area), "features": dict(zip(
                              ["typology", "kind", "era", "stories", "lot", "rules", "amendment", "business"], feats[i]))})
    overlaps = max((shed(a).intersection(shed(b)).area for a, b in itertools.combinations(route, 2)), default=0)

    # 4. The waypoint list: HC-G2's corner lots, by street and number.
    def addr_key(gpin):
        p = by_gpin[gpin]
        a = p["address"] or ""
        num_, _, street = a.partition(" ")
        if not a and p["streets"]:
            street = p["streets"][0][0]
        return (street, int(num_) if num_.isdigit() else 0)
    corners = sorted((p["gpin"] for p in parcels if p["corner"] and p["zone"] == "HC-G2"), key=addr_key)

    result = {
        "version": 1,
        "rules": RULES,
        "mix": {"source": "Policy Brief No. 2 (Ghent Streets), Section III: \"50% food/beverage (cafés and restaurants), 30% "
                          "retail goods, 20% personal/professional services. This mix is an assumption.\" The split within "
                          "each is ours.", "groups": MIX, "within": WITHIN},
        "by_right_max_sqft": BY_RIGHT_MAX_SQFT,
        "parcels": out,
        "tours": [{"id": "six", "title": "Show me around",
                   "note": "Six of the proposal's ground floors, chosen to differ from each other as much as they can (the "
                           "building, its age and size, what it becomes, why it may) and placed so that no stop can see "
                           "another's.",
                   "stops": [{"waypoint": w["id"]} for w in waypoints]}],
        "waypoints": waypoints,
        "corners": corners,
        "check": {"tour_shed_overlap_m2": round(overlaps, 2)},
    }
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as f:
        json.dump(result, f, separators=(",", ":"))
    want = sorted({w["gpin"] for w in waypoints} | set(corners) | {p["gpin"] for p in parcels if p["case"]})
    os.makedirs(os.path.join(ROOT, "target/ghent"), exist_ok=True)
    with open(os.path.join(ROOT, "target/ghent/photo_wanted.json"), "w") as f:
        json.dump(want, f)

    from collections import Counter
    st = Counter(r["status"] for r in out.values())
    print("status:", dict(st))
    print("rules (any status):", {k: sum(k in r["reasons"] for r in out.values()) for k in RULES})
    print("converted by rule:", {k: sum(k in r["reasons"] for r in out.values() if r["status"] == "converted") for k in RULES})
    conv = [r for r in out.values() if r["status"] == "converted"]
    print("converted in HC-G2:", sum(r["amendment"] for r in conv), "outside:", sum(not r["amendment"] for r in conv))
    print("typology:", dict(Counter(r["typology"] for r in conv)), " kind:", dict(Counter(r["kind"] for r in conv)))
    print("tour:", [(by_gpin[w["gpin"]]["address"], out[w["gpin"]]["typology"], out[w["gpin"]]["business"], w["shed_m2"]) for w in waypoints])
    print("tour shed overlap (m²):", round(overlaps, 2), " corners in list:", len(corners))


if __name__ == "__main__":
    main()
