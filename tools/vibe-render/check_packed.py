#!/usr/bin/env python3
"""Fail if a published .glb isn't packed (see pack_glb.py), or if its
street-level detail table is malformed.

An unpacked cadquery GLB draws one call per B-rep face (12k-18k on the real
scenarios), which is what choked laptops and phones in both the gallery's
<model-viewer> and walk.html. This keeps that from coming back unnoticed.

The detail table (pack_glb.add_detail) is what walk.html builds windows and
storefronts from; a table whose byte sizes don't match its counts, or whose
openings and shops point at walls that aren't there, would draw nonsense.

    python check_packed.py public/vibe-render/*.glb
"""

import sys

import numpy as np

from pack_glb import read_glb

MAX_PRIMITIVES = 64


def detail_problems(gltf, bin_chunk, table):
    """Empty when the detail table is absent or well formed; else what's wrong."""
    d = table.get("detail")
    if not d:
        return []
    if d.get("version") != 2:
        return [f"detail version {d.get('version')}"]
    views = gltf["bufferViews"]
    out = []
    w, o = d["walls"], d["openings"]
    if views[w["bufferView"]]["byteLength"] != w["count"] * 9 * 4:
        out.append("walls table size doesn't match its count")
    if views[o["bufferView"]]["byteLength"] != o["count"] * 6 * 2:
        out.append("openings table size doesn't match its count")
    if out:
        return out
    v = views[o["bufferView"]]
    ops = np.frombuffer(bin_chunk, np.uint16, o["count"] * 6, v.get("byteOffset", 0)).reshape(-1, 6)
    if o["count"] and ops[:, 0].max() >= w["count"]:
        out.append("an opening points at a wall that isn't there")
    if any(row[0] >= w["count"] or not row[1] < row[3] < row[2] for row in d["shops"]["rows"]):
        out.append("a shop front points at a missing wall, or its door isn't inside it")
    return out


failed = False
for path in sys.argv[1:]:
    gltf, bin_chunk = read_glb(path)
    prims = sum(len(m["primitives"]) for m in gltf.get("meshes", []))
    table = (gltf.get("nodes") or [{}])[0].get("extras", {}).get("streetSmarts")
    problems = detail_problems(gltf, bin_chunk, table) if table else []
    ok = prims <= MAX_PRIMITIVES and table and table.get("version") == 1 and table.get("elements") and not problems
    d = (table or {}).get("detail")
    extra = f", {d['openings']['count']} openings, {len(d['shops']['rows'])} shop fronts" if d else ""
    print(f"{'ok' if ok else 'FAIL'} {path}: {prims} primitives, {len(table['elements']) if table else 0} elements{extra}"
          + (f" ({'; '.join(problems)})" if problems else ""))
    failed |= not ok
sys.exit(1 if failed else 0)
