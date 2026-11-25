#!/usr/bin/env python3
"""Fail if a published .glb isn't packed (see pack_glb.py).

An unpacked cadquery GLB draws one call per B-rep face (12k-18k on the real
scenarios), which is what choked laptops and phones in both the gallery's
<model-viewer> and walk.html. This keeps that from coming back unnoticed.

    python check_packed.py public/vibe-render/*.glb
"""

import sys

from pack_glb import read_glb

MAX_PRIMITIVES = 64

failed = False
for path in sys.argv[1:]:
    gltf, _ = read_glb(path)
    prims = sum(len(m["primitives"]) for m in gltf.get("meshes", []))
    table = (gltf.get("nodes") or [{}])[0].get("extras", {}).get("streetSmarts")
    ok = prims <= MAX_PRIMITIVES and table and table.get("version") == 1 and table.get("elements")
    print(f"{'ok' if ok else 'FAIL'} {path}: {prims} primitives, {len(table['elements']) if table else 0} elements")
    failed |= not ok
sys.exit(1 if failed else 0)
