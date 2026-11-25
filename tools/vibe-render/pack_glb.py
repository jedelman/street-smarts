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
        elements.append([intern(rec.get(k)) for k in ("group", "kind", "source", "typology")] + [rec.get("floors")])
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
            "version": 1, "fields": ["group", "kind", "source", "typology", "floors"],
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
