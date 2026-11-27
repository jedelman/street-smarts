"""
build.py: Ghent Streets' model and site files, from the survey (facts) and the proposal (choices).

Writes, in the contract every street-smarts site shares (public/sites/README.md):
  public/sites/ghent-streets/model.glb     packed by tools/vibe-render/pack_glb.py's own pack(): the ground (lots,
                                           streets, sidewalks, the HC-G2 and League lines), every building, the
                                           City's inventory trees, the detail table (walls, windows, doors, and
                                           each converted ground floor), facts for every place, and the tour's
                                           waypoints as glTF camera nodes
  public/sites/ghent-streets/site.json     the bundle: model, numbers, tours, panels
  public/sites/ghent-streets/numbers.json  the proposal's counts, for the panels

Data inferred where not provided:
  - heights: the City's building "Elevation" field (survey.py); where it's missing, from the assessor's stories,
    else two stories for a house and one for anything small;
  - roofs: the City has footprints, not roof shapes. A house that nearly fills its rotated bounding rectangle gets
    a hip or a gable (seeded by its footprint, so it never changes); everything else is flat;
  - windows and doors: laid out in bays along each wall, a front door on the face to the street. Not surveyed:
    "doors and windows stay where they are" means the proposal adds nothing to and takes nothing from them;
  - sidewalks: a strip along each street;
  - trees: each where the City's inventory puts it; height and crown from its trunk diameter (the explorer's
    allometry), coloured for late November (ours).

Run:  tools/vibe-render/.venv/bin/python tools/ghent/build.py [--check]
      --check: rebuild in memory and fail if the committed site.json or numbers.json differ.
"""

import hashlib
import json
import math
import os
import struct
import sys

import numpy as np
import shapely
from shapely.geometry import LineString, MultiPolygon, Point, Polygon
from shapely.ops import unary_union

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), "../.."))
sys.path.insert(0, os.path.join(ROOT, "tools/vibe-render"))
import pack_glb  # noqa: E402

sys.path.insert(0, os.path.dirname(__file__))
import chorus as ghent_chorus  # noqa: E402

SURVEY = os.path.join(ROOT, "data/ghent/survey.json")
PROPOSAL = os.path.join(ROOT, "data/ghent/proposal.json")
SITE_DIR = os.path.join(ROOT, "public/sites/ghent-streets")
SQFT = 10.7639

HOUSE_USES = ("Single Family", "Duplex", "Triplex", "Fourplex")
STREET_HALF = {"arterial/collector": 6.0, "residential": 4.5}   # half a carriageway, metres (ours)
SIDEWALK_W = 2.1
Y = {"street": 0.02, "lot": 0.08, "sidewalk": 0.14, "line": 0.15}
FIRST_FLOOR = 0.6     # a Ghent house's first floor stands above the ground (ours)
FLOOR_M = 3.1

COLORS = {
    "lawn": "#8b995c", "lawn_converted": "#a9a55e", "street": "#57544f", "sidewalk": "#cdc6b6",
    "line_hcg2": "#c99a3e", "line_league": "#efe9da",
    "roof_slate": "#4b4f53", "roof_brown": "#5d4f45", "roof_red": "#6e3d30", "roof_flat": "#6f6a62",
    "wall_brick": "#8f4c37", "wall_brick2": "#a0603f", "wall_cream": "#e6dbc0", "wall_white": "#efeadf",
    "wall_blue": "#9eabb1", "wall_sage": "#a8ae8d", "wall_yellow": "#d8c38a",
    "wall_stone": "#cdc1a6", "wall_context": "#b3ad9f", "roof_context": "#8b867c",
}
HOUSE_WALLS = ["wall_brick", "wall_brick2", "wall_cream", "wall_white", "wall_blue", "wall_sage", "wall_yellow"]
# The City's inventory forms (survey.tree_form), for late November in Norfolk (ours): oaks bronze, maples and
# sweetgums turned, crape myrtles and dogwoods red, conifers and magnolias green.
TREE_COLORS = {"oak": "#7d6a3c", "plane": "#a38c4c", "maple": "#b65a2f", "round": "#6f7d42", "crape": "#a2583a",
               "small": "#9c4f3a", "pine": "#3f5e45", "cone": "#3d5a41", "evergreen": "#41613f"}
TREE_FORMS = {  # the explorer's: height and crown radius = min(a + b·cm, cap)
    "oak": ((4.9, 0.23, 24), (1.2, 0.105, 10)), "plane": ((5, 0.24, 26), (1.2, 0.1, 10)),
    "maple": ((4, 0.25, 20), (1.1, 0.11, 8)), "round": ((4, 0.24, 18), (1.1, 0.1, 7.5)),
    "crape": ((2.5, 0.22, 9), (0.9, 0.1, 4)), "small": ((2.5, 0.17, 9), (1, 0.1, 5)),
    "pine": ((5, 0.33, 28), (1, 0.06, 5)), "cone": ((3, 0.24, 15), (0.8, 0.05, 3)),
    "evergreen": ((3.5, 0.2, 16), (1.2, 0.13, 11)),
}
TREE_UNKNOWN_DBH = 7
CAMERA_YFOV = 1.08


def h01(*key):
    return int(hashlib.sha1(":".join(map(str, key)).encode()).hexdigest()[:12], 16) / 16 ** 12


def srgb_linear(hexc):
    c = np.array([int(hexc[i:i + 2], 16) / 255 for i in (1, 3, 5)])
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def gl(x, y, up=0.0):
    """Survey metres (east, north) to glTF axes (x east, y up, z south)."""
    return (x, up, -y)


def polys(g):
    return [g] if isinstance(g, Polygon) else [q for q in getattr(g, "geoms", []) if isinstance(q, Polygon)]


def ring_area(pts):
    return 0.5 * sum(pts[i][0] * pts[(i + 1) % len(pts)][1] - pts[(i + 1) % len(pts)][0] * pts[i][1] for i in range(len(pts)))


class Unpacked:
    """An unpacked glTF in memory, one node per element, for pack_glb.pack() to pack."""

    def __init__(self):
        self.gltf = {"asset": {"version": "2.0", "generator": "street-smarts tools/ghent/build.py"}, "scene": 0,
                     "scenes": [{"nodes": []}], "nodes": [], "meshes": [], "materials": [], "accessors": [],
                     "bufferViews": [], "buffers": [{"byteLength": 0}]}
        self.blob = bytearray()
        self.mats = {}
        self.records = {}

    def material(self, name):
        if name not in self.mats:
            rgb = srgb_linear(COLORS[name])
            self.gltf["materials"].append({"name": name, "pbrMetallicRoughness": {
                "baseColorFactor": [float(rgb[0]), float(rgb[1]), float(rgb[2]), 1.0], "metallicFactor": 0.0, "roughnessFactor": 1.0}})
            self.mats[name] = len(self.gltf["materials"]) - 1
        return self.mats[name]

    def _acc(self, arr, ctype, atype, target, minmax=False):
        self.blob.extend(b"\0" * (-len(self.blob) % 4))
        self.gltf["bufferViews"].append({"buffer": 0, "byteOffset": len(self.blob), "byteLength": arr.nbytes, "target": target})
        self.blob.extend(arr.tobytes())
        acc = {"bufferView": len(self.gltf["bufferViews"]) - 1, "componentType": ctype, "count": len(arr), "type": atype}
        if minmax:
            acc["min"], acc["max"] = arr.min(axis=0).tolist(), arr.max(axis=0).tolist()
        self.gltf["accessors"].append(acc)
        return len(self.gltf["accessors"]) - 1

    def node(self, name, record, prims):
        """prims: {material name: Geo}. record: the element's group, kind, source, typology, floors."""
        out = []
        for mat, geo in prims.items():
            if not geo.idx:
                continue
            pos = np.array(geo.pos, np.float32)
            nrm = np.array(geo.nrm, np.float32)
            idx = np.array(geo.idx, np.uint32)
            out.append({"attributes": {"POSITION": self._acc(pos, 5126, "VEC3", 34962, True),
                                       "NORMAL": self._acc(nrm, 5126, "VEC3", 34962)},
                        "indices": self._acc(idx, 5125, "SCALAR", 34963), "material": self.material(mat), "mode": 4})
        if not out:
            return
        self.gltf["meshes"].append({"primitives": out})
        self.gltf["nodes"].append({"name": name, "mesh": len(self.gltf["meshes"]) - 1})
        self.gltf["scenes"][0]["nodes"].append(len(self.gltf["nodes"]) - 1)
        self.records[name] = record

    def packed(self):
        self.gltf["buffers"][0]["byteLength"] = len(self.blob)
        return pack_glb.pack(self.gltf, bytes(self.blob), self.records)


class Geo:
    """Triangles with flat normals, indexed per planar face."""

    def __init__(self):
        self.pos, self.nrm, self.idx = [], [], []

    def face(self, pts, normal):
        """A convex planar polygon (glTF coordinates), wound to face `normal`."""
        pts = [np.asarray(p, float) for p in pts]
        n = np.asarray(normal, float)
        n = n / (np.linalg.norm(n) or 1)
        cross = np.cross(pts[1] - pts[0], pts[2] - pts[0])
        if np.dot(cross, n) < 0:
            pts = pts[::-1]
        base = len(self.pos)
        for p in pts:
            self.pos.append(p.tolist())
            self.nrm.append(n.tolist())
        for k in range(1, len(pts) - 1):
            self.idx += [base, base + k, base + k + 1]

    def flat(self, poly, y):
        """A horizontal polygon (survey metres, holes allowed) at height y, facing up."""
        if poly.is_empty or poly.area < 0.05:
            return
        tris = shapely.constrained_delaunay_triangles(poly)
        base = len(self.pos)
        verts = {}
        for t in getattr(tris, "geoms", []):
            cs = list(t.exterior.coords)[:3]
            if Polygon(cs).area < 1e-4:
                continue
            ids = []
            for x, yy in cs:
                key = (round(x, 3), round(yy, 3))
                if key not in verts:
                    verts[key] = len(self.pos)
                    self.pos.append([x, y, -yy])
                    self.nrm.append([0.0, 1.0, 0.0])
                ids.append(verts[key])
            # Wound counter-clockwise seen from above (glTF: +y up, z south).
            (ax, _, az), (bx, _, bz), (cx, _, cz) = (self.pos[i] for i in ids)
            if (bx - ax) * (cz - az) - (bz - az) * (cx - ax) > 0:
                ids = [ids[0], ids[2], ids[1]]
            self.idx += ids
        del base


def walls(geo, pts, y0, y1):
    """Vertical walls round a ring (survey metres), from y0 to y1, each facing out."""
    ccw = ring_area(pts) > 0
    for i in range(len(pts)):
        a, c = pts[i], pts[(i + 1) % len(pts)]
        dx, dy = c[0] - a[0], c[1] - a[1]
        L = math.hypot(dx, dy)
        if L < 0.05:
            continue
        nx, ny = (dy / L, -dx / L) if ccw else (-dy / L, dx / L)
        geo.face([gl(*a, y0), gl(*c, y0), gl(*c, y1), gl(*a, y1)], (nx, 0, -ny))


def roof_form(b, parcel, poly):
    """The roof a building gets (inferred): ('hip'|'gable', rect, pitch) or ('flat',)."""
    if b["type"] not in ("residential", "garage", "other") or poly.area > 420:
        return ("flat",)
    house = parcel and (parcel["use"] or "").startswith(HOUSE_USES)
    if b["type"] == "residential" and parcel and not house and poly.area > 160:
        return ("flat",)  # an apartment building
    rect = poly.minimum_rotated_rectangle
    if rect.area <= 0 or poly.area / rect.area < 0.74:
        return ("flat",)
    key = ",".join(f"{v:.1f}" for v in poly.bounds)
    form = "gable" if h01(key, "roof") < 0.45 or b["type"] == "garage" else "hip"
    pitch = math.radians(28 + 12 * h01(key, "pitch"))
    return (form, rect, pitch)


def roof(geo, rect, form, pitch, eave, overhang=0.3):
    """A hip or gable roof over a rotated rectangle; returns the ridge height."""
    cs = [np.array(c) for c in list(rect.exterior.coords)[:4]]
    e0, e1 = cs[1] - cs[0], cs[2] - cs[1]
    if np.linalg.norm(e0) < np.linalg.norm(e1):
        cs = cs[1:] + cs[:1]
        e0, e1 = cs[1] - cs[0], cs[2] - cs[1]
    L, W = np.linalg.norm(e0), np.linalg.norm(e1)
    u, v = e0 / L, e1 / W
    c0 = cs[0] - u * overhang - v * overhang
    L2, W2 = L + 2 * overhang, W + 2 * overhang
    P = lambda a, b, h: gl(*(c0 + u * a + v * b), h)
    rise = min(W2 / 2 * math.tan(pitch), 5.5)
    top = eave + rise
    inset = W2 / 2 if form == "hip" else 0.0
    inset = min(inset, L2 / 2 - 0.01)
    r0, r1 = P(inset, W2 / 2, top), P(L2 - inset, W2 / 2, top)
    a0, a1, b1, b0 = P(0, 0, eave), P(L2, 0, eave), P(L2, W2, eave), P(0, W2, eave)
    up = lambda pts: np.cross(np.subtract(pts[1], pts[0]), np.subtract(pts[2], pts[0]))

    def face(pts):
        n = up(pts)
        if n[1] < 0:
            n = -n
        geo.face(pts, n)
    face([a0, a1, r1, r0])
    face([b1, b0, r0, r1])
    if form == "hip":
        face([a1, b1, r1])
        face([b0, a0, r0])
    return top, (P(0, 0, eave), P(0, W2, eave), r0, P(L2, 0, eave), P(L2, W2, eave), r1) if form == "gable" else None


def main(check=False):
    S = json.load(open(SURVEY))
    PR = json.load(open(PROPOSAL))
    parcels = {p["gpin"]: p for p in S["parcels"]}
    plan = PR["parcels"]
    U = Unpacked()
    facts = {}

    def addr_title(a):
        return " ".join(w.capitalize() if not w[:1].isdigit() else w for w in (a or "").split()) or None

    league = unary_union([Polygon(r) for r in S["league"]])
    hcg2 = unary_union([Polygon(r).buffer(0) for r in S["hcg2"]])

    # --- the ground: streets (each name its own element), sidewalks, lots, the two lines
    by_name = {}
    for s in S["streets"]:
        by_name.setdefault(s["name"], {"row": s["row"], "lines": []})["lines"].append(LineString(s["line"]))
    covered = Polygon()
    street_polys = []
    for name in sorted(by_name, key=lambda n: (by_name[n]["row"] != "arterial/collector", n)):
        st = by_name[name]
        half = STREET_HALF.get(st["row"], 4.5)
        g = unary_union([ln.buffer(half, quad_segs=3, cap_style="flat") for ln in st["lines"]]).simplify(0.25).difference(covered)
        covered = covered.union(g)
        street_polys.append((name, st["row"], g))
        geo = Geo()
        for q in polys(g):
            geo.flat(q, Y["street"])
        src = f"street:{name}"
        U.node(f"street_{len(street_polys)}", {"group": "streets", "kind": "street_arterial" if st["row"] == "arterial/collector" else "street_residential", "source": src}, {"street": geo})
        nice = addr_title(name)
        facts[src] = {"name": nice, "arrival": f"Onto {nice}{', an arterial or collector street' if st['row'] == 'arterial/collector' else ''}.", "answer": [
            f"{nice}: {'an arterial or collector street' if st['row'] == 'arterial/collector' else 'a residential street'}, in the City's classification."
            + (" NFK2050 points a neighborhood's shops toward streets like this one (p. 69)." if st["row"] == "arterial/collector" else "")
            + (" Jason's maximal proposal opens its whole frontage." if name in ("GRAYDON AVENUE", "BLOW STREET") else "")]}
    walks = unary_union([ln.buffer(STREET_HALF.get(by_name[n]["row"], 4.5) + SIDEWALK_W, quad_segs=3, cap_style="flat")
                         for n in by_name for ln in by_name[n]["lines"]]).simplify(0.25).difference(covered).buffer(0)
    geo = Geo()
    for q in polys(walks):
        geo.flat(q, Y["sidewalk"])
    U.node("sidewalks", {"group": "sidewalks", "kind": "sidewalk", "source": "sidewalks"}, {"sidewalk": geo})
    facts["sidewalks"] = {"name": "the sidewalk", "answer": ["A sidewalk, drawn along every street: the City maps its sidewalks as lines, and this model infers their width."]}
    taken = covered.union(walks)

    for gpin, p in parcels.items():
        lot = Polygon(p["poly"]).buffer(0).difference(taken)
        if lot.is_empty:
            continue
        r = plan.get(gpin, {})
        geo = Geo()
        for q in polys(lot):
            geo.flat(q, Y["lot"])
        U.node(f"lot_{gpin}", {"group": "lots", "kind": "lot_converted" if r.get("status") == "converted" else "lot",
                               "source": f"parcel:{gpin}"}, {"lawn_converted" if r.get("status") == "converted" else "lawn": geo})

    for key, area, color, width, label in (("boundary_hcg2", hcg2, "line_hcg2", 0.8,
                                            "The edge of HC-G2, Ghent's historic district, whose use table (Table 3.6.11) the amendment would change."),
                                           ("boundary_league", league, "line_league", 0.5,
                                            "The edge of the Ghent Neighborhood League's area (the City's civic-league map): the proposal's reach.")):
        geo = Geo()
        for q in polys(area):
            band = q.exterior.buffer(width / 2, cap_style="flat")
            for b in polys(band):
                geo.flat(b, Y["line"])
        U.node(key, {"group": "boundaries", "kind": key, "source": key}, {color: geo})
        facts[key] = {"name": "this line", "answer": [label], "arrival": label}

    # --- the buildings
    detail = {"walls": [], "openings": [], "shops": []}
    # Every footprint, to keep a front door from opening into a neighbour's wall.
    all_polys = [Polygon(b["poly"]).buffer(0) if len(b["poly"]) >= 3 else Polygon() for b in S["buildings"]]
    # The survey cuts a row's footprint at each lot line; a slice past every lot line (a bay or stoop drawn over the
    # right-of-way) belongs to no parcel. Where it overlaps a parcel's building and shares its City height (so it was
    # one footprint), it goes back into that building, so the facade is its outer face.
    owned = [i for i, b in enumerate(S["buildings"]) if b["parcel"] and not all_polys[i].is_empty]
    otree = shapely.STRtree([all_polys[i] for i in owned])
    merged_into = {}
    for i, b in enumerate(S["buildings"]):
        pg = all_polys[i]
        if b["parcel"] or not b["in_area"] or pg.is_empty:
            continue
        grown, best = pg.buffer(0.3), None
        for k in otree.query(grown):
            j = owned[k]
            shared = grown.intersection(all_polys[j]).area
            if shared > 0.5 and S["buildings"][j]["h"] == b["h"] and (best is None or shared > best[0]):
                best = (shared, j)
        if best:
            j = best[1]
            u = all_polys[j].union(pg.buffer(0.05)).buffer(-0.05)
            if isinstance(u, MultiPolygon):
                u = max(u.geoms, key=lambda q: q.area)
            all_polys[j] = u.simplify(0.1)
            all_polys[i] = Polygon()
            merged_into[i] = j
    btree = shapely.STRtree(all_polys)

    def blocked(bi, a, c, n):
        """True when the ground 2 m out from the middle of wall a-c lies inside another building."""
        probe = Point((a[0] + c[0]) / 2 + n[0] * 2.0, (a[1] + c[1]) / 2 + n[1] * 2.0)
        return any(j != bi and all_polys[j].contains(probe) for j in btree.query(probe))
    step_m = 1.4  # the walk stands people and doorsteps 1.6 m out; a door is clear if 1.4 m and 2.2 m out are
    bstats = {"roofs_inferred": 0, "heights_inferred": 0, "buildings": 0}
    for bi, b in enumerate(S["buildings"]):
        if bi in merged_into:
            continue
        poly = all_polys[bi]
        if poly.is_empty or poly.area < 3:
            continue
        p = parcels.get(b["parcel"]) if b["parcel"] else None
        r = plan.get(b["parcel"], {}) if b["parcel"] else {}
        main_b = r.get("building") == bi
        house = p and (p["use"] or "").startswith(HOUSE_USES) and b["type"] == "residential"
        stories = p["stories"] if (p and main_b and p["stories"]) else None
        form = roof_form(b, p if main_b else None, poly)
        bstats["buildings"] += 1
        # The eave: from the assessor's stories, else from the City's height less the roof's rise, else a guess.
        if b["type"] == "garage":
            eave = 2.8
        elif stories:
            eave = (FIRST_FLOOR if house else 0) + stories * FLOOR_M + 0.3
        elif b["h"]:
            est_rise = 0 if form[0] == "flat" else min(5.5, math.sqrt(form[1].area) / 2 * math.tan(form[2]))
            eave = max(2.8, b["h"] - est_rise * 0.8)
        else:
            eave = 6.8 if poly.area > 45 else 3.0
            bstats["heights_inferred"] += 1
        context = not b["in_area"]
        wall_c = "wall_context" if context else (
            "wall_stone" if b["type"] in ("institutional", "university", "hospital", "school", "library", "public safety")
            else "wall_brick" if b["type"] == "commercial" else HOUSE_WALLS[int(h01(bi, "paint") * len(HOUSE_WALLS))])
        roof_c = "roof_context" if context else ["roof_slate", "roof_brown", "roof_red"][int(h01(bi, "roof") * 3)] if form[0] != "flat" else "roof_flat"
        wg, rg = Geo(), Geo()
        ring = list(poly.exterior.coords)[:-1]
        walls(wg, ring, 0.0, eave)
        top = eave
        if form[0] == "flat":
            rg.flat(poly, eave)
        else:
            bstats["roofs_inferred"] += 1
            top, gab = roof(rg, form[1], form[0], form[2], eave)
            if gab:  # the gable ends, in the wall's colour
                for tri in (gab[:3], gab[3:]):
                    n = np.cross(np.subtract(tri[1], tri[0]), np.subtract(tri[2], tri[0]))
                    wg.face(list(tri), n)
        floors = stories or max(1, round((eave - (FIRST_FLOOR if house else 0)) / FLOOR_M))
        src = f"parcel:{b['parcel']}" if b["parcel"] else f"building:{bi}"
        U.node(f"building_{bi}", {"group": "buildings", "kind": "building_shaped" if not context else "context", "source": src,
                                  "typology": b["type"], "floors": int(math.ceil(floors)), "area": round(poly.area, 1)},
               {wall_c: wg, roof_c: rg})

        # Street-level detail: the walls of buildings in the area, their windows and doors (inferred), and the
        # converted ground floors. Not for garages, sheds or towers.
        if context or b["type"] in ("garage", "water tower", "storage tank") or poly.area < 25:
            continue
        ccw = ring_area(ring) > 0
        front = r.get("front") if main_b else None
        first = FIRST_FLOOR if house else 0.0
        nf = int(max(1, round(floors)))
        bay = 2.7 if house else 3.2
        win_w, win_h = (0.85, 1.55) if house else (1.15, 1.7)
        # The face to the street: the proposal's front for its parcel's main building, else the longest wall
        # that faces a street within 25 m.
        wall_ids = []
        for i in range(len(ring)):
            a, c = ring[i], ring[(i + 1) % len(ring)]
            dx, dy = c[0] - a[0], c[1] - a[1]
            L = math.hypot(dx, dy)
            if L < 2.2:
                wall_ids.append(None)
                continue
            nx, ny = (dy / L, -dx / L) if ccw else (-dy / L, dx / L)
            mid = Point((a[0] + c[0]) / 2 + nx * 3, (a[1] + c[1]) / 2 + ny * 3)
            hidden = blocked(bi, a, c, (nx, ny))
            public = covered.distance(mid) < 22 and not hidden
            detail["walls"].append({"ax": a[0], "ay": a[1], "dx": dx / L, "dy": dy / L, "nx": nx, "ny": ny, "length": L,
                                    "public": public, "on_hole": False, "building": src})
            wall_ids.append((len(detail["walls"]) - 1, L, public, a, c))
        # Which wall is the front: the one nearest the proposal's front edge, else the longest public one.
        front_w = None
        if front:
            fm = ((front["a"][0] + front["c"][0]) / 2, (front["a"][1] + front["c"][1]) / 2)
            best = None
            for w in wall_ids:
                if not w:
                    continue
                wm = ((w[3][0] + w[4][0]) / 2, (w[3][1] + w[4][1]) / 2)
                d = math.dist(wm, fm)
                if best is None or d < best[0]:
                    best = (d, w)
            front_w = best[1] if best and best[0] < 4 and best[1][2] else None
        if not front_w:
            pub = [w for w in wall_ids if w and w[2]]
            front_w = max(pub, key=lambda w: w[1]) if pub else None
        door_at = None
        for w in wall_ids:
            if not w:
                continue
            wid, L, public, a, c = w
            n = max(0, int((L - 0.6) // bay))
            if n == 0:
                continue
            slots = [(k + 0.5) * L / n for k in range(n)]
            is_front = front_w is not None and wid == front_w[0]
            if is_front:
                # The door: a seeded slot, or the nearest one to it whose doorstep isn't inside a building.
                k = min(n - 1, max(0, int(h01(bi, "door") * n))) if n > 2 else n // 2
                ux, uy = (c[0] - a[0]) / L, (c[1] - a[1]) / L
                nx, ny = (uy, -ux) if ccw else (-uy, ux)
                for j in sorted(range(n), key=lambda j: abs(j - k)):
                    steps = [Point(a[0] + ux * slots[j] + nx * o, a[1] + uy * slots[j] + ny * o) for o in (step_m, 2.2)]
                    if not any(all_polys[q].contains(st) for st in steps for q in btree.query(st)):
                        door_at = slots[j]
                        break
            for f in range(nf):
                y0 = first + f * FLOOR_M
                for s in slots:
                    if is_front and f == 0 and s == door_at:
                        detail["openings"].append({"wall": wid, "kind": "door", "floor": 0, "along": s, "z": max(0.0, first - 0.15),
                                                   "width": 1.0, "height": 2.2})
                        continue
                    detail["openings"].append({"wall": wid, "kind": "window", "floor": f, "along": s, "z": y0 + 0.8,
                                               "width": min(win_w, L / n - 0.5), "height": win_h})
        # The converted ground floor.
        if main_b and r.get("status") == "converted" and front_w and door_at is not None:
            wid, L = front_w[0], front_w[1]
            typ = r["typology"]
            span = {"house": L - 0.6, "flats": 9.0, "institution": 12.0, "storefront": min(L - 0.7, 12.5)}[typ]
            span = max(2.4, min(span, L - 0.6, 12.5))
            lo = max(0.3, min(door_at - span / 2, L - 0.3 - span))
            detail["shops"].append({"wall": wid, "lo": lo, "hi": lo + span, "door": door_at, "door_width": 1.0, "door_height": 2.2,
                                    "typology": typ, "business": r["business"], "source": src, "first": first})

    # --- the trees
    trees = []
    for k, (x, y, form, dbh, name) in enumerate(S["trees"]):
        (ha, hb, hc), (ra, rb, rc) = TREE_FORMS.get(form, TREE_FORMS["round"])
        cm = (dbh or TREE_UNKNOWN_DBH) * 2.54
        j = 0.9 + 0.2 * h01(x, y, "j")
        trees.append({"x": x, "y": y, "form": form, "id": f"tree:{k}", "height_m": round(min(ha + hb * cm, hc) * j, 2),
                      "crown_radius_m": round(min(ra + rb * cm, rc) * (0.9 + 0.2 * h01(x, y, "r")), 2),
                      "typology": name or None, "floors": dbh or None})
    # One sentence for every tree, filled from its element (typology: the species; floors: the trunk's diameter).
    facts["group:trees"] = {"name": "{typology|a tree}", "answer": [
        "{typology|A tree}{floors?, # inches across the trunk}: where the City's tree inventory puts it. "
        "Its height and crown are drawn from that diameter, so they're approximate."]}

    # --- facts for every parcel
    rules = PR["rules"]
    noun = {"Single Family - Detached": "a house", "Single Family - Attached": "a row house", "Duplex": "a duplex",
            "Triplex": "a triplex", "Fourplex": "a fourplex", "Apartment": "an apartment building", "Condominium": "condominiums",
            "Co-op": "a co-op", "Commercial": "a commercial building", "Non-Taxable": "a tax-exempt property",
            "Vacant Land": "vacant land", "Residential Outbuilding": "an outbuilding"}
    biz_noun = {"coffee": "a coffee shop", "bistro": "a bistro", "bakery": "a bakery", "books": "a bookshop",
                "greengrocer": "a greengrocer's", "florist": "a florist's", "hardware": "a hardware store", "bikes": "a bike shop",
                "records": "a record shop", "hairdresser": "a hairdresser's", "accountant": "an accountant's office",
                "architect": "an architects' studio", "design": "a design studio", "tailor": "a tailor's",
                "dentist": "a dentist's office", "gallery": "an art gallery", "yoga": "a yoga studio", "barber": "a barbershop",
                "cleaners": "a dry cleaner's"}
    for gpin, p in parcels.items():
        r = plan.get(gpin, {})
        name = addr_title(p["address"]) or "This lot"
        what = noun.get(p["use"], "a lot")
        built = f", built {p['built']}" if p["built"] and p["built"] > 1700 else ""
        lines = [f"{name}: {what}{built}."]
        why = "; ".join(rules[k]["label"] for k in r.get("reasons", []))
        st = r.get("status")
        if st == "converted":
            lines.append(f"In the proposal, its ground floor holds {biz_noun[r['business']]} ({r['use']}), up to "
                         f"{r['unit_sqft']:,} sq ft, behind the doors and windows it has now.")
            lines.append(f"Why it may: {why}.")
            lines.append("Today HC-G2's use table (Norfolk Zoning Ordinance, Table 3.6.11) leaves this use blank: prohibited."
                         if r["amendment"] else f"It's zoned {p['zone']}, not HC-G2, so the HC-G2 amendment alone wouldn't reach it.")
        elif st == "kept":
            lines.append(f"It's {r['why']}: the proposal leaves it as it is.")
        elif st == "existing":
            lines.append(f"Already commercial (the assessor's class: {p['cls']}).")
        elif st == "nothing-to-adapt":
            lines.append(f"The proposal would allow it ({why}), but there's no building here to adapt.")
        else:
            lines.append("Not in the proposal: NFK2050 keeps neighborhood interiors residential (p. 69).")
        rec = {"name": name, "prompt": f"Read {name}", "answer": lines, "status": st}
        if p.get("photo"):
            rec["photo"] = p["photo"]
        if p.get("sv"):
            lat, lng, hd = p["sv"]
            rec["streetview"] = f"https://www.google.com/maps/@?api=1&map_action=pano&viewpoint={lat},{lng}&heading={hd}"
        if p.get("pid"):
            rec["card"] = f"https://air.norfolk.gov/#/property/{p['pid']}"
        if p.get("case") and p["case"] != "801":
            rec["case"] = f"https://jason-edelman.org/ghent-streets/case-studies/{p['case']}.html"
        if st == "converted":
            rec["business"] = r["business"]
            rec["typology"] = r["typology"]
        facts[f"parcel:{gpin}"] = rec
    facts["group:buildings"] = {"name": "a building", "prompt": "Look at the building", "answer": [
        "A building (the City's type: {typology|not given}), from the City's building footprints. Its height is the City's where it "
        "has one; its roof and windows are inferred."]}

    gltf, binc = U.packed()
    gltf, binc = pack_glb.add_trees(gltf, binc, trees, colors=TREE_COLORS)
    gltf, binc = pack_glb.add_detail(gltf, binc, detail)
    table = gltf["nodes"][0]["extras"]["streetSmarts"]
    table["facts"] = facts
    table["site"] = {"id": "ghent-streets", "pipeline": "adaptive reuse (tools/ghent)", "inferred": [
        "building heights where the City's field is missing", "roof shapes", "windows and doors", "sidewalks", "tree sizes"]}

    # What the map draws over the plan, at map scale (a line 0.8 m wide on the ground is lost at 1.7 m a pixel):
    # glTF x/z, so the walk needs no conversion.
    ring_xz = lambda q: [[round(x, 1), round(-y, 1)] for x, y in list(q.exterior.simplify(1.0).coords)]
    main_of = {g: r["building"] for g, r in plan.items() if r.get("status") == "converted" and r.get("building") is not None}
    dots = []
    for g, bi in main_of.items():
        q = all_polys[bi] if not all_polys[bi].is_empty else Polygon(S["buildings"][bi]["poly"])
        c = q.representative_point()
        dots.append([round(c.x, 1), round(-c.y, 1)])
    table["overlays"] = [
        {"id": "converted", "label": f"Ground floors that open ({len(dots)})", "color": "#ff8a4c", "radius": 2.4, "points": dots},
        {"id": "hcg2", "label": "HC-G2, the district the amendment covers", "color": "#f2c14e", "width": 2.2,
         "rings": [ring_xz(q) for q in polys(hcg2)]},
        {"id": "league", "label": "The Ghent Neighborhood League's area", "color": "#ffffff", "width": 1.6, "dash": [5, 4],
         "rings": [ring_xz(q) for q in polys(league)]},
        {"id": "tour", "label": "The tour's stops", "color": "#2a2116", "fill": "#fbf6ec",
         "labels": [{"at": [round(w["look"][0], 1), round(-w["look"][1], 1)], "text": str(k + 1)} for k, w in enumerate(PR["waypoints"])]},
    ]

    # The tour's waypoints, as glTF camera nodes: where to stand (a sparrow's eye, 2.2 m up) and what to look at.
    gltf["cameras"] = [{"type": "perspective", "perspective": {"yfov": CAMERA_YFOV, "znear": 0.1, "zfar": 3000}}]
    for w in PR["waypoints"]:
        p = parcels[w["gpin"]]
        eye = np.array(gl(*w["stand"], 2.2))
        look = np.array(gl(*w["look"], 2.0))
        f = look - eye
        f /= np.linalg.norm(f)
        rgt = np.cross(f, [0, 1, 0])
        rgt /= np.linalg.norm(rgt)
        upv = np.cross(rgt, f)
        m = np.column_stack([rgt, upv, -f])
        qw = math.sqrt(max(0, 1 + m[0, 0] + m[1, 1] + m[2, 2])) / 2
        qx = math.copysign(math.sqrt(max(0, 1 + m[0, 0] - m[1, 1] - m[2, 2])) / 2, m[2, 1] - m[1, 2])
        qy = math.copysign(math.sqrt(max(0, 1 - m[0, 0] + m[1, 1] - m[2, 2])) / 2, m[0, 2] - m[2, 0])
        qz = math.copysign(math.sqrt(max(0, 1 - m[0, 0] - m[1, 1] + m[2, 2])) / 2, m[1, 0] - m[0, 1])
        gltf["nodes"].append({"name": f"waypoint:{w['id']}", "camera": 0, "translation": [round(float(v), 3) for v in eye],
                              "rotation": [round(qx, 6), round(qy, 6), round(qz, 6), round(qw, 6)],
                              "extras": {"waypoint": w["id"], "source": f"parcel:{w['gpin']}", "title": addr_title(p["address"]),
                                         "look": [round(float(v), 3) for v in look]}})
        gltf["nodes"][0]["children"].append(len(gltf["nodes"]) - 1)

    # --- numbers and the bundle
    def corner_entry(g):
        p, r = parcels[g], plan.get(g, {})
        on = addr_title(p["streets"][0][0]) if p["streets"] else None
        label = addr_title(p["address"]) or (f"A lot on {on}" if on else "A lot")
        street = label.split(" ", 1)[1] if p["address"] and " " in label else on
        note = {"converted": f"opens as {biz_noun[r['business']]}" if r.get("business") else "opens",
                "existing": "already commercial", "nothing-to-adapt": "no building to adapt",
                "kept": f"{r.get('why', 'kept')}, left as it is"}.get(r.get("status"), "not in the proposal")
        return {"element": f"parcel:{g}", "label": label, "street": street, "note": note}

    conv = [r for r in plan.values() if r["status"] == "converted"]
    numbers = {
        "version": 1,
        "parcels": len(parcels),
        "status": {k: sum(r["status"] == k for r in plan.values()) for k in ("converted", "existing", "kept", "nothing-to-adapt", "not-eligible")},
        "converted_in_hcg2": sum(r["amendment"] for r in conv),
        "converted_by_rule": {k: sum(k in r["reasons"] for r in conv) for k in rules},
        "converted_by_kind": {k: sum(r["kind"] == k for r in conv) for k in ("food", "retail", "services")},
        "converted_by_typology": {k: sum(r["typology"] == k for r in conv) for k in ("house", "flats", "institution", "storefront")},
        "converted_by_business": {k: sum(r["business"] == k for r in conv) for k in sorted({r["business"] for r in conv})},
        "unit_sqft_total": sum(r["unit_sqft"] for r in conv),
        "buildings": bstats["buildings"], "roofs_inferred": bstats["roofs_inferred"],
        "trees": len(trees), "corners": len(PR["corners"]),
        "built": S["meta"]["built"],
    }
    site = {
        "version": 1,
        "id": "ghent-streets",
        "title": "Ghent Streets",
        "place": "the Ghent Neighborhood League's area, Norfolk, Virginia",
        "pipeline": "adaptive reuse",
        "model": "model.glb",
        "numbers": "numbers.json",
        "renderers": {"money": "money.js", "corners": "corners.js", "place": "place.js"},
        "live": ["place"],
        "views": False,
        "tours": [{**t, "max": len(t["stops"])} for t in PR["tours"]],
        "corners": [corner_entry(g) for g in PR["corners"]],
        "spawn": {"waypoint": PR["waypoints"][0]["id"]},
    }
    # Who says what, where: the chorus's places, from the proposal.
    src = lambda gs: {f"parcel:{g}" for g in gs}
    conv = {g: r for g, r in plan.items() if r.get("status") == "converted"}
    sets = {
        "converted": src(conv),
        "converted_houses": src(g for g, r in conv.items() if r["typology"] == "house"),
        "converted_flats": src(g for g, r in conv.items() if r["typology"] == "flats"),
        "food": src(g for g, r in conv.items() if r["kind"] == "food"),
        "hcg2": src(g for g, r in conv.items() if r["amendment"]),
        "colley": src(g for g, p in parcels.items() if any(n == "COLLEY AVENUE" for n, _ in p["streets"])),
        "corners": src(PR["corners"]),
        "beyond_plan": src(g for g, r in conv.items() if not any(k.startswith("nfk2050") for k in r["reasons"])),
    }
    chorus_text = json.dumps(ghent_chorus.build(sets), indent=1, ensure_ascii=False) + "\n"
    site["chorus"] = "chorus.json"
    site_text = json.dumps(site, indent=1, ensure_ascii=False) + "\n"
    numbers_text = json.dumps(numbers, indent=1) + "\n"
    if check:
        bad = []
        for fn, text in (("site.json", site_text), ("numbers.json", numbers_text), ("chorus.json", chorus_text)):
            path = os.path.join(SITE_DIR, fn)
            if not os.path.exists(path) or open(path).read() != text:
                bad.append(fn)
        if bad:
            sys.exit(f"stale: {', '.join(bad)} (run tools/ghent/build.py and commit)")
        print("site.json, numbers.json and chorus.json are current")
    os.makedirs(SITE_DIR, exist_ok=True)
    pack_glb.write_glb(os.path.join(SITE_DIR, "model.glb"), gltf, binc)
    if not check:
        open(os.path.join(SITE_DIR, "site.json"), "w").write(site_text)
        open(os.path.join(SITE_DIR, "numbers.json"), "w").write(numbers_text)
        open(os.path.join(SITE_DIR, "chorus.json"), "w").write(chorus_text)
    size = os.path.getsize(os.path.join(SITE_DIR, "model.glb"))
    print(f"model.glb {size / 1e6:.2f} MB: {len(table['elements'])} elements, {len(gltf['meshes'])} meshes, "
          f"{len(detail['walls'])} walls, {len(detail['openings'])} openings, {len(detail['shops'])} shops, {len(trees)} trees, "
          f"{len(PR['waypoints'])} waypoints; {bstats['roofs_inferred']} roofs and {bstats['heights_inferred']} heights inferred")


if __name__ == "__main__":
    main(check="--check" in sys.argv)
