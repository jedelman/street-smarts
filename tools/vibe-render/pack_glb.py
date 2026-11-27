#!/usr/bin/env python3
"""Pack a cadquery-written .glb into one mesh per material.

cadquery's glTF writer gives every solid its own node and every B-rep face
its own primitive. On `clean_baseline` that is 11,987 primitives for 68,576
triangles, and each primitive is a draw call in a browser: about 24,000 a
frame once three.js draws the double-sided translucent ones twice. That, not
the triangle count, is what choked laptops and phones. Two thirds of the file
was JSON describing those primitives (5.6 MB of 8.2 MB).

Packing bakes every node's transform into its vertices and concatenates the
triangles of each material into a single primitive, so a viewer draws one
call per material (about 15). Nothing about the geometry changes.

What a solid was survives in `extras`, so the walk page can say what you
touched:
  - the root node's `extras.streetSmarts.elements` lists every solid as
    `[group, kind, source, typology, floors]`, the first four as indices
    into `extras.streetSmarts.strings` (-1 when absent). source is the id of
    the record it came from, which names the patterns that placed it;
    typology and floors are a building's;
  - each mesh's `extras.ranges` is a flat list of
    `element, first_triangle, triangle_count` triples, in triangle order.

Materials at or above OPAQUE_ALPHA become opaque. Translucency used to let
the separate solids show through each other; one merged mesh can't be
depth-sorted against itself, and a building you walk around should be solid.
So does any translucent layer whose footprint takes in all the solid
geometry (in practice the context buildings; see pack()). Ground slabs,
halos and the ghostly low-confidence kinds keep their alpha.

Standard library + numpy only. Usable on its own for any cadquery .glb:
    python pack_glb.py in.glb out.glb
"""

import json
import re
import struct
import sys

import numpy as np

OPAQUE_ALPHA = 0.8

_COMPONENT = {5120: np.int8, 5121: np.uint8, 5122: np.int16, 5123: np.uint16, 5125: np.uint32, 5126: np.float32}
_WIDTH = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}
_NAME = re.compile(r"^(.*?)_(\d+)_(.*)$")  # export_glb's f"{group}_{n}_{kind}"


def read_glb(path):
    with open(path, "rb") as f:
        data = f.read()
    magic, _version, _length = struct.unpack_from("<III", data, 0)
    if magic != 0x46546C67:
        raise ValueError(f"{path} is not a binary glTF file")
    json_len, json_type = struct.unpack_from("<II", data, 12)
    if json_type != 0x4E4F534A:
        raise ValueError(f"{path}: first chunk is not JSON")
    gltf = json.loads(data[20:20 + json_len])
    bin_start = 20 + json_len
    bin_chunk = b""
    if bin_start < len(data):
        bin_len, bin_type = struct.unpack_from("<II", data, bin_start)
        if bin_type != 0x004E4942:
            raise ValueError(f"{path}: second chunk is not BIN")
        bin_chunk = data[bin_start + 8:bin_start + 8 + bin_len]
    return gltf, bin_chunk


def write_glb(path, gltf, bin_chunk):
    js = json.dumps(gltf, separators=(",", ":")).encode()
    js += b" " * (-len(js) % 4)
    bin_chunk += b"\0" * (-len(bin_chunk) % 4)
    total = 12 + 8 + len(js) + 8 + len(bin_chunk)
    with open(path, "wb") as f:
        f.write(struct.pack("<III", 0x46546C67, 2, total))
        f.write(struct.pack("<II", len(js), 0x4E4F534A))
        f.write(js)
        f.write(struct.pack("<II", len(bin_chunk), 0x004E4942))
        f.write(bin_chunk)


def read_accessor(gltf, bin_chunk, index):
    acc = gltf["accessors"][index]
    if acc.get("sparse"):
        raise ValueError("sparse accessors aren't supported")
    view = gltf["bufferViews"][acc["bufferView"]]
    dtype = np.dtype(_COMPONENT[acc["componentType"]])
    width = _WIDTH[acc["type"]]
    start = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = view.get("byteStride") or dtype.itemsize * width
    count = acc["count"]
    if stride == dtype.itemsize * width:
        arr = np.frombuffer(bin_chunk, dtype=dtype, count=count * width, offset=start)
        return arr.reshape(count, width) if width > 1 else arr
    rows = np.lib.stride_tricks.as_strided(
        np.frombuffer(bin_chunk, dtype=np.uint8, offset=start),
        shape=(count, dtype.itemsize * width), strides=(stride, 1))
    arr = np.ascontiguousarray(rows).view(dtype)
    return arr.reshape(count, width) if width > 1 else arr.reshape(count)


def node_matrix(node):
    if "matrix" in node:
        return np.array(node["matrix"], dtype=np.float64).reshape(4, 4).T  # glTF is column-major
    m = np.eye(4)
    x, y, z, w = node.get("rotation", (0, 0, 0, 1))
    m[:3, :3] = [
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ]
    m[:3, :3] *= np.array(node.get("scale", (1, 1, 1)))[None, :]
    m[:3, 3] = node.get("translation", (0, 0, 0))
    return m


def element_of(name, elements_by_name):
    """The element record for a node name: export_glb's own table if given,
    else whatever the name itself says."""
    if elements_by_name and name in elements_by_name:
        return elements_by_name[name]
    m = _NAME.match(name or "")
    if m:
        return {"group": m.group(1), "kind": m.group(3)}
    return {"group": "other", "kind": name or ""}


def pack(gltf, bin_chunk, elements_by_name=None, opaque_alpha=OPAQUE_ALPHA):
    """Return (gltf, bin) with one mesh per material. See the module doc."""
    nodes = gltf.get("nodes", [])
    scene = gltf["scenes"][gltf.get("scene", 0)]

    # Every mesh node with its world matrix, in document order.
    placed = []
    def walk(i, parent):
        world = parent @ node_matrix(nodes[i])
        if "mesh" in nodes[i]:
            placed.append((i, world))
        for c in nodes[i].get("children", []):
            walk(c, world)
    for root in scene["nodes"]:
        walk(root, np.eye(4))
    placed.sort(key=lambda p: p[0])

    strings, string_index = [], {}
    def intern(s):
        if s is None:
            return -1
        if s not in string_index:
            string_index[s] = len(strings)
            strings.append(s)
        return string_index[s]

    elements = []
    per_mat = {}  # material -> {"pos": [], "nrm": [], "idx": [], "nverts": int, "ntris": int, "ranges": []}
    for node_index, world in placed:
        node = nodes[node_index]
        rec = element_of(node.get("name"), elements_by_name)
        e = len(elements)
        elements.append([intern(rec.get(k)) for k in ("group", "kind", "source", "typology")] + [rec.get("floors"), intern(rec.get("uses")), rec.get("area")])
        rot = world[:3, :3]
        normal_m = np.linalg.inv(rot).T
        for prim in gltf["meshes"][node["mesh"]]["primitives"]:
            if prim.get("mode", 4) != 4:
                raise ValueError(f"node {node.get('name')}: only triangle lists are supported")
            pos = read_accessor(gltf, bin_chunk, prim["attributes"]["POSITION"]).astype(np.float64)
            pos = pos @ rot.T + world[:3, 3]
            if "NORMAL" in prim["attributes"]:
                nrm = read_accessor(gltf, bin_chunk, prim["attributes"]["NORMAL"]).astype(np.float64) @ normal_m.T
                lens = np.linalg.norm(nrm, axis=1, keepdims=True)
                nrm = nrm / np.where(lens > 0, lens, 1)
            else:
                raise ValueError(f"node {node.get('name')}: no normals")
            if "indices" in prim:
                idx = read_accessor(gltf, bin_chunk, prim["indices"]).astype(np.uint32)
            else:
                idx = np.arange(len(pos), dtype=np.uint32)
            mat = prim.get("material", -1)
            slot = per_mat.setdefault(mat, {"pos": [], "nrm": [], "idx": [], "nverts": 0, "ntris": 0, "ranges": []})
            ntri = len(idx) // 3
            r = slot["ranges"]
            if r and r[-3] == e and r[-2] + r[-1] == slot["ntris"]:
                r[-1] += ntri  # the same element, still contiguous
            else:
                r += [e, slot["ntris"], ntri]
            slot["pos"].append(pos.astype(np.float32))
            slot["nrm"].append(nrm.astype(np.float32))
            slot["idx"].append(idx + slot["nverts"])
            slot["nverts"] += len(pos)
            slot["ntris"] += ntri

    out = {
        "asset": {"version": "2.0", "generator": "street-smarts pack_glb.py (from cadquery)"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": "street-smarts", "children": [], "extras": {"streetSmarts": {
            "version": 1, "fields": ["group", "kind", "source", "typology", "floors", "uses", "area"],
            "strings": strings, "elements": elements}}}],
        "meshes": [],
        "materials": [],
        "accessors": [],
        "bufferViews": [],
        "buffers": [{"byteLength": 0}],
    }
    blob = bytearray()

    def add_view(arr, target):
        blob.extend(b"\0" * (-len(blob) % 4))
        out["bufferViews"].append({"buffer": 0, "byteOffset": len(blob), "byteLength": arr.nbytes, "target": target})
        blob.extend(arr.tobytes())
        return len(out["bufferViews"]) - 1

    def add_accessor(arr, ctype, atype, target, minmax=False):
        acc = {"bufferView": add_view(arr, target), "componentType": ctype, "count": len(arr), "type": atype}
        if minmax:
            acc["min"] = arr.min(axis=0).tolist()
            acc["max"] = arr.max(axis=0).tolist()
        out["accessors"].append(acc)
        return len(out["accessors"]) - 1

    src_materials = gltf.get("materials", [])

    def alpha_of(mat):
        m = src_materials[mat] if mat >= 0 else {}
        rgba = m.get("pbrMetallicRoughness", {}).get("baseColorFactor", [1, 1, 1, 1])
        return rgba[3] if m.get("alphaMode") == "BLEND" else 1.0

    # <model-viewer> hides a translucent mesh that is flat for its size and
    # encloses the rest of the model, taking it for a baked floor shadow (its
    # findBakedShadows/checkBakedShadows). One merged mesh of every context
    # building is exactly that shape, so a translucent layer whose footprint
    # takes in all the solid geometry is made opaque too.
    bounds = {mat: (np.min([p.min(axis=0) for p in s["pos"]], axis=0), np.max([p.max(axis=0) for p in s["pos"]], axis=0))
              for mat, s in per_mat.items()}
    solid = [bounds[m] for m in per_mat if alpha_of(m) >= opaque_alpha]
    force_opaque = set()
    if solid:
        lo = np.min([b[0] for b in solid], axis=0)
        hi = np.max([b[1] for b in solid], axis=0)
        for mat, (blo, bhi) in bounds.items():
            if alpha_of(mat) < opaque_alpha and blo[0] <= lo[0] and blo[2] <= lo[2] and bhi[0] >= hi[0] and bhi[2] >= hi[2]:
                force_opaque.add(mat)

    uses_emissive_strength = False
    for mat in sorted(per_mat):
        slot = per_mat[mat]
        pos = np.concatenate(slot["pos"])
        nrm = np.concatenate(slot["nrm"])
        idx = np.concatenate(slot["idx"])
        idx = idx.astype(np.uint16) if slot["nverts"] <= 65535 else idx
        prim = {
            "attributes": {
                "POSITION": add_accessor(pos, 5126, "VEC3", 34962, minmax=True),
                "NORMAL": add_accessor(nrm, 5126, "VEC3", 34962),
            },
            "indices": add_accessor(idx, 5123 if idx.dtype == np.uint16 else 5125, "SCALAR", 34963),
            "mode": 4,
        }
        if mat >= 0:
            m = json.loads(json.dumps(src_materials[mat]))
            pbr = m.setdefault("pbrMetallicRoughness", {})
            rgba = pbr.get("baseColorFactor", [1, 1, 1, 1])
            if m.get("alphaMode") == "BLEND" and (rgba[3] >= opaque_alpha or mat in force_opaque):
                m.pop("alphaMode")
                pbr["baseColorFactor"] = rgba[:3] + [1.0]
            uses_emissive_strength |= "KHR_materials_emissive_strength" in m.get("extensions", {})
            out["materials"].append(m)
            prim["material"] = len(out["materials"]) - 1
        out["meshes"].append({"name": f"material_{len(out['meshes'])}", "primitives": [prim], "extras": {"ranges": slot["ranges"]}})
        out["nodes"].append({"name": out["meshes"][-1]["name"], "mesh": len(out["meshes"]) - 1})
        out["nodes"][0]["children"].append(len(out["nodes"]) - 1)

    if not out["materials"]:
        del out["materials"]
    if uses_emissive_strength:
        out["extensionsUsed"] = ["KHR_materials_emissive_strength"]
    out["buffers"][0]["byteLength"] = len(blob)
    return out, bytes(blob)


# --- Trees -------------------------------------------------------------------
#
# P171's trees go in as two instanced meshes (EXT_mesh_gpu_instancing): one
# crown and one trunk, each drawn once for every tree. Two thousand trees cost
# two draw calls and about 130 KB, where solids from cadquery would cost two
# thousand nodes and megabytes. A viewer without the extension shows one tree
# at the origin; three.js, so <model-viewer> and the walk, has it.
#
# Each tree is an element too: the instancing nodes' `extras.elementBase` is
# the index of the first tree's element, and instance i is element base + i.

TREE_CROWN_COLORS = {  # sRGB; P171's forms, a shade apart so a square's ring reads against a grove
    "grove": "#3f6b35",
    "avenue": "#4d7d3e",
    "square": "#5b8c43",
    "umbrella": "#35602e",
    "cypress": "#a3562b",  # P64's bald cypress, russet: it turns before it drops its needles, as in late November
}
# Bald cypress stands tall and narrow, on a trunk that flares at the water.
CYPRESS_CROWN = (0.72, 1.75)  # crown width and height, in crown radii
# Other narrow forms, from a city's tree inventory (tools/ghent): conifers stand taller than they're wide.
FORM_CROWN = {"cypress": CYPRESS_CROWN, "cone": (0.75, 2.0), "pine": (0.9, 1.35)}
TREE_TRUNK_COLOR = "#5b4433"
CROWN_SQUASH = 0.9  # crown height over crown width
TRUNK_BASE_Y = -0.05  # just below the meadow's top, so every trunk meets the ground


def _srgb_to_linear(hex_color):
    c = np.array([int(hex_color[i:i + 2], 16) / 255 for i in (1, 3, 5)])
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def _hash01(text, salt):
    """A stable number in [0, 1) from a tree's id, so a re-render doesn't
    re-roll its turn or shade."""
    import zlib
    return (zlib.crc32(f"{salt}:{text}".encode()) & 0xFFFFFFFF) / 2**32


def unit_crown(subdivide=True):
    """A lumpy icosphere of radius about 1, centred on the origin, Y up, flat
    shaded: (positions, normals), three vertices per triangle, 80 triangles
    (20 with `subdivide=False`, for the isometric's matplotlib)."""
    t = (1 + 5 ** 0.5) / 2
    v = [(-1, t, 0), (1, t, 0), (-1, -t, 0), (1, -t, 0), (0, -1, t), (0, 1, t), (0, -1, -t), (0, 1, -t),
         (t, 0, -1), (t, 0, 1), (-t, 0, -1), (-t, 0, 1)]
    f = [(0, 11, 5), (0, 5, 1), (0, 1, 7), (0, 7, 10), (0, 10, 11), (1, 5, 9), (5, 11, 4), (11, 10, 2), (10, 7, 6),
         (7, 1, 8), (3, 9, 4), (3, 4, 2), (3, 2, 6), (3, 6, 8), (3, 8, 9), (4, 9, 5), (2, 4, 11), (6, 2, 10),
         (8, 6, 7), (9, 8, 1)]
    verts = [np.array(p, dtype=np.float64) / np.linalg.norm(p) for p in v]
    mid = {}
    def midpoint(a, b):
        key = (min(a, b), max(a, b))
        if key not in mid:
            m = verts[a] + verts[b]
            verts.append(m / np.linalg.norm(m))
            mid[key] = len(verts) - 1
        return mid[key]
    faces = []
    for a, b, c in f:
        if not subdivide:
            faces.append((a, b, c))
            continue
        ab, bc, ca = midpoint(a, b), midpoint(b, c), midpoint(c, a)
        faces += [(a, ab, ca), (b, bc, ab), (c, ca, bc), (ab, bc, ca)]
    lumps = np.array([1 + 0.12 * (_hash01(str(i), "lump") - 0.5) * 2 for i in range(len(verts))])
    pts = np.array(verts) * lumps[:, None]
    pts[:, 1] = np.where(pts[:, 1] < -0.35, -0.35 + (pts[:, 1] + 0.35) * 0.5, pts[:, 1])  # a flatter underside
    tri = pts[np.array(faces)]  # (80, 3, 3)
    n = np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0])
    n /= np.linalg.norm(n, axis=1, keepdims=True)
    return tri.reshape(-1, 3), np.repeat(n, 3, axis=0)


def unit_trunk(sides=5):
    """An open prism of radius 1 from y=0 to y=1, flat shaded, 10 triangles."""
    pos, nrm = [], []
    for i in range(sides):
        a0, a1 = 2 * np.pi * i / sides, 2 * np.pi * (i + 1) / sides
        p0, p1 = np.array([np.cos(a0), 0, np.sin(a0)]), np.array([np.cos(a1), 0, np.sin(a1)])
        q0, q1 = p0 + [0, 1, 0], p1 + [0, 1, 0]
        n = np.cross(q0 - p0, p1 - p0)
        n /= np.linalg.norm(n)
        for tri in ((p0, q0, p1), (p1, q0, q1)):
            pos += tri
            nrm += [n, n, n]
    return np.array(pos), np.array(nrm)


def tree_instances(trees, colors=None):
    """Per-tree transforms for `unit_crown` and `unit_trunk`, glTF axes (Y up,
    -Z north). `trees` are dicts with local metres `x`, `y` (east, north),
    `height_m`, `crown_radius_m`, `form`, `id`. Returns a dict of float32
    arrays: crown_t, crown_s, trunk_t, trunk_s (n, 3), rot (n, 4), color (n, 3,
    linear). `colors` maps a form to its sRGB crown colour, over TREE_CROWN_COLORS."""
    palette = {**TREE_CROWN_COLORS, **(colors or {})}
    n = len(trees)
    out = {k: np.zeros((n, 3), np.float32) for k in ("crown_t", "crown_s", "trunk_t", "trunk_s", "color")}
    out["rot"] = np.zeros((n, 4), np.float32)
    for i, t in enumerate(trees):
        r, h = t["crown_radius_m"], t["height_m"]
        wide, tall = FORM_CROWN.get(t["form"], (1.0, CROWN_SQUASH))
        cy = h - tall * r
        out["crown_t"][i] = (t["x"], cy, -t["y"])
        out["crown_s"][i] = (wide * r, tall * r, wide * r)
        tw = (0.24 + 0.06 * r) if t["form"] == "cypress" else (0.14 + 0.05 * r)
        out["trunk_t"][i] = (t["x"], TRUNK_BASE_Y, -t["y"])
        out["trunk_s"][i] = (tw, cy - TRUNK_BASE_Y, tw)
        yaw = 2 * np.pi * _hash01(t["id"], "yaw")
        out["rot"][i] = (0, np.sin(yaw / 2), 0, np.cos(yaw / 2))
        shade = 0.85 + 0.3 * _hash01(t["id"], "shade")
        out["color"][i] = np.clip(_srgb_to_linear(palette.get(t["form"], palette["grove"])) * shade, 0, 1)
    return out


def add_trees(gltf, bin_chunk, trees, colors=None):
    """Append `trees` (see `tree_instances`) to a packed glTF as two instanced
    meshes plus one element each. Returns (gltf, bin)."""
    if not trees:
        return gltf, bin_chunk
    blob = bytearray(bin_chunk)
    root = gltf["nodes"][0]
    table = root["extras"]["streetSmarts"]
    strings = table["strings"]
    def intern(s):
        if s is None:
            return -1
        if s not in strings:
            strings.append(s)
        return strings.index(s)
    base = len(table["elements"])
    for t in trees:
        table["elements"].append([intern("trees"), intern(t["form"]), intern(t["id"]), intern(t.get("typology")), t.get("floors"), -1, None])

    def add_accessor(arr, ctype, atype, target=None, minmax=False):
        blob.extend(b"\0" * (-len(blob) % 4))
        view = {"buffer": 0, "byteOffset": len(blob), "byteLength": arr.nbytes}
        if target:
            view["target"] = target
        gltf["bufferViews"].append(view)
        blob.extend(arr.tobytes())
        acc = {"bufferView": len(gltf["bufferViews"]) - 1, "componentType": ctype, "count": len(arr), "type": atype}
        if minmax:
            acc["min"], acc["max"] = arr.min(axis=0).tolist(), arr.max(axis=0).tolist()
        gltf["accessors"].append(acc)
        return len(gltf["accessors"]) - 1

    inst = tree_instances(trees, colors)
    gltf.setdefault("materials", [])
    for name, (pos, nrm), color, t_key, s_key, colors in (
        ("tree_crowns", unit_crown(), [1, 1, 1, 1], "crown_t", "crown_s", True),
        ("tree_trunks", unit_trunk(), list(_srgb_to_linear(TREE_TRUNK_COLOR)) + [1], "trunk_t", "trunk_s", False),
    ):
        gltf["materials"].append({"name": name, "pbrMetallicRoughness": {
            "baseColorFactor": [float(c) for c in color], "metallicFactor": 0.0, "roughnessFactor": 1.0}})
        prim = {"attributes": {
            "POSITION": add_accessor(pos.astype(np.float32), 5126, "VEC3", 34962, minmax=True),
            "NORMAL": add_accessor(nrm.astype(np.float32), 5126, "VEC3", 34962)},
            "material": len(gltf["materials"]) - 1, "mode": 4}
        gltf["meshes"].append({"name": name, "primitives": [prim]})
        attrs = {
            "TRANSLATION": add_accessor(inst[t_key], 5126, "VEC3"),
            "ROTATION": add_accessor(inst["rot"], 5126, "VEC4"),
            "SCALE": add_accessor(inst[s_key], 5126, "VEC3"),
        }
        if colors:
            attrs["_COLOR_0"] = add_accessor(inst["color"], 5126, "VEC3")
        gltf["nodes"].append({"name": name, "mesh": len(gltf["meshes"]) - 1,
                              "extensions": {"EXT_mesh_gpu_instancing": {"attributes": attrs}},
                              "extras": {"elementBase": base, "count": len(trees)}})
        root["children"].append(len(gltf["nodes"]) - 1)
    used = gltf.setdefault("extensionsUsed", [])
    if "EXT_mesh_gpu_instancing" not in used:
        used.append("EXT_mesh_gpu_instancing")
    gltf["buffers"][0]["byteLength"] = len(blob)
    return gltf, bytes(blob)


def add_trees_file(path, trees):
    gltf, bin_chunk = read_glb(path)
    gltf, bin_chunk = add_trees(gltf, bin_chunk, trees)
    write_glb(path, gltf, bin_chunk)
    return len(trees)


# --- Street-level detail ------------------------------------------------------
#
# render.frontage_detail's walls, openings and shop fronts, for the walk to
# build windows and storefronts from (instanced, and only near the camera).
# Two binary tables and a short list, referenced from the root's
# `extras.streetSmarts.detail`:
#   walls     float32 x 9: ax, az, dx, dz, nx, nz, length, flags, building
#             (glTF axes: x east, z south; a wall runs from (ax, az) along
#             (dx, dz) for `length` metres, facing (nx, nz); flags 1 = public,
#             2 = on a courtyard; building = index into `strings`)
#   openings  uint16 x 6: wall, kind | floor << 2 (kind 0 window, 1 door,
#             2 a P87 shop door), along, bottom, width, height (centimetres)
#   shops     rows of [wall, lo, hi, door, door_width, door_height, arcade,
#             p87] (metres along the wall; arcade 1 when a P119 arcade
#             covers it; p87 1 when its door is a P87 shop door), then, where a
#             pipeline chose them, [typology, business, source, first]: how the
#             front meets the street ("storefront", or "house", "flats" and
#             "institution", which keep their doors and windows), the business,
#             the element's source and the first floor's height (tools/ghent)
# About 120 KB for clean_baseline's 9,784 openings.

def add_detail(gltf, bin_chunk, detail):
    walls, openings, shops = detail.get("walls") or [], detail.get("openings") or [], detail.get("shops") or []
    if not walls:
        return gltf, bin_chunk
    blob = bytearray(bin_chunk)
    table = gltf["nodes"][0]["extras"]["streetSmarts"]
    strings = table["strings"]
    def intern(text):
        if text not in strings:
            strings.append(text)
        return strings.index(text)

    def add_view(arr):
        blob.extend(b"\0" * (-len(blob) % 4))
        gltf["bufferViews"].append({"buffer": 0, "byteOffset": len(blob), "byteLength": arr.nbytes})
        blob.extend(arr.tobytes())
        return len(gltf["bufferViews"]) - 1

    w = np.array([[x["ax"], -x["ay"], x["dx"], -x["dy"], x["nx"], -x["ny"], x["length"],
                   (1 if x["public"] else 0) | (2 if x["on_hole"] else 0), intern(x["building"])] for x in walls], np.float32)
    cm = lambda v: int(min(65535, max(0, round(v * 100))))
    kinds = {"window": 0, "door": 1, "shop_door": 2}
    o = np.array([[x["wall"], kinds.get(x["kind"], 0) | (x["floor"] << 2),
                   cm(x["along"]), cm(x["z"]), cm(x["width"]), cm(x["height"])] for x in openings], np.uint16)
    r2 = lambda v: round(float(v), 2)
    chosen = any("typology" in x for x in shops)
    extra = (lambda x: [x.get("typology"), x.get("business"), x.get("source"), r2(x.get("first") or 0)]) if chosen else (lambda x: [])
    table["detail"] = {
        "version": 2,
        "walls": {"bufferView": add_view(w), "count": len(walls),
                  "fields": ["ax", "az", "dx", "dz", "nx", "nz", "length", "flags", "building"]},
        "openings": {"bufferView": add_view(o), "count": len(openings),
                     "fields": ["wall", "kindFloor", "along", "bottom", "width", "height"]},
        "shops": {"fields": ["wall", "lo", "hi", "door", "doorWidth", "doorHeight", "arcade", "p87"]
                  + (["typology", "business", "source", "first"] if chosen else []),
                  "rows": [[x["wall"], r2(x["lo"]), r2(x["hi"]), r2(x["door"]), r2(x["door_width"]), r2(x["door_height"]),
                            1 if x.get("arcade") else 0, 1 if x.get("p87") else 0] + extra(x) for x in shops]},
    }
    gltf["buffers"][0]["byteLength"] = len(blob)
    return gltf, bytes(blob)


def add_detail_file(path, detail):
    gltf, bin_chunk = read_glb(path)
    gltf, bin_chunk = add_detail(gltf, bin_chunk, detail)
    write_glb(path, gltf, bin_chunk)


# --- Transit -------------------------------------------------------------------
#
# P34's lines, for the walk's tram: `extras.streetSmarts.transit` lists each
# line as {id, kind, points: [[x, z], ...] in glTF axes (y is the ground),
# stations: [{id, at (metres along the line), length}]}.

def add_transit(gltf, lines):
    if not lines:
        return gltf
    table = gltf["nodes"][0]["extras"]["streetSmarts"]
    table["transit"] = [{
        "id": l["id"], "kind": l.get("kind"),
        "points": [[round(x, 2), round(-y, 2)] for x, y in l["points"]],
        "stations": [{"id": s["id"], "at": round(s["at"], 2), "length": round(s["length"], 2)} for s in l["stations"]],
    } for l in lines]
    return gltf


def add_transit_file(path, lines):
    if not lines:
        return
    gltf, bin_chunk = read_glb(path)
    write_glb(path, add_transit(gltf, lines), bin_chunk)


def pack_file(in_path, out_path=None, elements_by_name=None):
    gltf, bin_chunk = read_glb(in_path)
    before = sum(len(m["primitives"]) for m in gltf.get("meshes", []))
    packed, blob = pack(gltf, bin_chunk, elements_by_name)
    write_glb(out_path or in_path, packed, blob)
    return before, len(packed["meshes"])


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage: pack_glb.py in.glb out.glb")
    before, after = pack_file(sys.argv[1], sys.argv[2])
    print(f"packed {sys.argv[1]}: {before} primitives -> {after}")
