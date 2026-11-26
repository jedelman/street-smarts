#!/usr/bin/env python3
"""Archive one iteration of the plan, so the walk and the landing page can
show it next to the others.

An iteration is a folder in the R2 bucket `street-smarts-archive`, served by
the site's Worker at /archive/<id>/ (worker/index.js):

    clean_baseline.glb   the whole site, as the walk loads it
    numbers.json         plan_numbers.py's measurements of it
    chorus.json          today's chorus on it (chorus_of), for eavesdropping
    isometric.png        the render, for the landing page's thumbnail

and archive/index.json lists every iteration, oldest first. Run this on a
finished render (scripts/vibe-render.sh's target/vibe-render, or any folder
holding clean_baseline.glb and clean_baseline.json) when a change is worth
keeping as a milestone; it never runs on its own.

    python archive_milestone.py --id el-born --title "El Born heights, scattered work" \\
        --date 2026-11-26 --pr 41 --summary "..." target/vibe-render

Needs CLOUDFLARE_API_TOKEN (R2 write) in the environment; the account is the
one in wrangler.toml. `--chorus-bin` is the chorus_of example (built by
scripts/vibe-render.sh); `--dry-run` prints what it would upload.
"""

import argparse
import json
import os
import pathlib
import subprocess
import sys
import urllib.error
import urllib.request

ACCOUNT = "543fde7feabd0af945bfae926ceea7ad"
BUCKET = "street-smarts-archive"
API = f"https://api.cloudflare.com/client/v4/accounts/{ACCOUNT}/r2/buckets/{BUCKET}/objects/"
HERE = pathlib.Path(__file__).resolve().parent


def request(method, key, body=None, content_type=None):
    req = urllib.request.Request(API + key, data=body, method=method)
    req.add_header("Authorization", "Bearer " + os.environ.get("CLOUDFLARE_API_TOKEN", ""))
    if content_type:
        req.add_header("Content-Type", content_type)
    try:
        with urllib.request.urlopen(req) as r:
            return r.read()
    except urllib.error.HTTPError as e:
        if method == "GET" and e.code == 404:
            return None
        raise SystemExit(f"{method} {key}: HTTP {e.code} {e.read()[:300]!r}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("render_dir")
    ap.add_argument("--id", required=True, help="short slug, e.g. el-born")
    ap.add_argument("--title", required=True)
    ap.add_argument("--date", required=True, help="YYYY-MM-DD the change shipped")
    ap.add_argument("--pr", type=int)
    ap.add_argument("--commit", help="the commit it was built from")
    ap.add_argument("--summary", required=True, help="one or two plain sentences: what changed")
    ap.add_argument("--chorus-bin", default=str(HERE.parents[1] / "target/release/examples/chorus_of"))
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    d = pathlib.Path(args.render_dir)
    glb, nbhd = d / "clean_baseline.glb", d / "clean_baseline.json"
    for f in (glb, nbhd):
        if not f.exists():
            raise SystemExit(f"missing {f}")
    numbers = subprocess.run([sys.executable, str(HERE / "plan_numbers.py"), str(nbhd), str(glb)], check=True, capture_output=True, text=True).stdout
    chorus = subprocess.run([args.chorus_bin, str(nbhd)], check=True, capture_output=True, text=True).stdout
    iso = d / "clean_baseline_isometric.png"

    files = [
        ("clean_baseline.glb", glb.read_bytes(), "model/gltf-binary"),
        ("numbers.json", numbers.encode(), "application/json"),
        ("chorus.json", chorus.encode(), "application/json"),
    ]
    if iso.exists():
        files.append(("isometric.png", iso.read_bytes(), "image/png"))

    n = json.loads(numbers)
    entry = {
        "id": args.id, "title": args.title, "date": args.date, "pr": args.pr, "commit": args.commit,
        "summary": args.summary, "files": [f for f, _, _ in files],
        "stats": {k: n.get(k) for k in ("buildings", "floors_median", "floors_max", "floor_area_m2", "live_m2", "work_m2",
                                          "play_m2", "shop_fronts", "trees", "transit_stations")},
    }
    current = request("GET", "archive/index.json")
    index = json.loads(current) if current else {"version": 1, "iterations": []}
    index["iterations"] = [i for i in index["iterations"] if i["id"] != args.id] + [entry]
    index["iterations"].sort(key=lambda i: (i["date"], i.get("pr") or 0))

    for name, body, ctype in files:
        key = f"archive/{args.id}/{name}"
        print(f"{'would put' if args.dry_run else 'put'} {key} ({len(body):,} bytes, {ctype})")
        if not args.dry_run:
            request("PUT", key, body, ctype)
    body = json.dumps(index, indent=1).encode()
    print(f"{'would put' if args.dry_run else 'put'} archive/index.json ({len(index['iterations'])} iterations)")
    if not args.dry_run:
        request("PUT", "archive/index.json", body, "application/json")


if __name__ == "__main__":
    main()
