#!/usr/bin/env python3
"""Archive one build of a site bundle (public/sites/<site>/), so its walk can show it next to the others: a
before and after for people to compare.

A build is a folder in the R2 bucket `street-smarts-archive`, served by the site's Worker at
/archive/<site>/<id>/ (worker/index.js), and archive/<site>/index.json lists them, oldest first. The site's
site.json names that index as `archive`; the walk's Iterations panel reads it. (Eastside's own milestones keep their
older layout and tool: tools/vibe-render/archive_milestone.py.)

The folder holds what the walk needs to load the build: model.glb, and beside it numbers.json and chorus.json if the
build has them, a thumb.jpg for the panel, and a showcase.json with its shots (tools/sparrow/showcase.mjs) if any.

    python tools/sparrow/archive_site.py --site ghent-streets --id ghent-first --page /ghent/ \\
        --title "Ghent Streets joins the walk" --date 2026-11-27 --pr 52 --commit f37fd77 --summary "..." <folder>

Writes through the Cloudflare API (R2 object PUT) for the account in wrangler.toml, with CLOUDFLARE_API_TOKEN from
the environment. `--dry-run` prints what it would upload. It never deletes.
"""

import argparse
import json
import mimetypes
import os
import pathlib
import urllib.error
import urllib.request

ACCOUNT = "543fde7feabd0af945bfae926ceea7ad"
BUCKET = "street-smarts-archive"
API = f"https://api.cloudflare.com/client/v4/accounts/{ACCOUNT}/r2/buckets/{BUCKET}/objects/"
TYPES = {".glb": "model/gltf-binary", ".json": "application/json", ".jpg": "image/jpeg", ".png": "image/png", ".webp": "image/webp"}


def request(method, key, body=None, content_type=None):
    req = urllib.request.Request(API + key, data=body, method=method)
    req.add_header("Authorization", "Bearer " + os.environ.get("CLOUDFLARE_API_TOKEN", "unset"))
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
    ap.add_argument("folder")
    ap.add_argument("--site", required=True, help="the bundle's id, e.g. ghent-streets")
    ap.add_argument("--id", required=True, help="a short slug for this build, e.g. ghent-first")
    ap.add_argument("--page", required=True, help="the site's page, e.g. /ghent/")
    ap.add_argument("--title", required=True)
    ap.add_argument("--date", required=True, help="YYYY-MM-DD the build shipped")
    ap.add_argument("--pr", type=int)
    ap.add_argument("--commit", help="the commit it was built from")
    ap.add_argument("--summary", required=True, help="one or two plain sentences: what it was")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    d = pathlib.Path(args.folder)
    if not (d / "model.glb").exists():
        raise SystemExit(f"missing {d / 'model.glb'}")
    names = [n for n in ("model.glb", "numbers.json", "chorus.json", "thumb.jpg", "showcase.json") if (d / n).exists()]
    shots = []
    if (d / "showcase.json").exists():
        shots = [s["image"] for s in json.loads((d / "showcase.json").read_text()).get("shots", []) if s.get("image")]
        names += [s for s in shots if (d / s).exists()]
    folder = f"archive/{args.site}/{args.id}/"
    entry = {"id": args.id, "title": args.title, "date": args.date, "pr": args.pr, "commit": args.commit,
             "summary": args.summary, "page": args.page, "folder": "/" + folder, "model": "model.glb",
             "files": names}
    for k, n in (("numbers", "numbers.json"), ("chorus", "chorus.json"), ("thumb", "thumb.jpg"), ("showcase", "showcase.json")):
        if n in names:
            entry[k] = n
    index_key = f"archive/{args.site}/index.json"
    current = request("GET", index_key)
    index = json.loads(current) if current else {"version": 1, "site": args.site, "iterations": []}
    index["iterations"] = [i for i in index["iterations"] if i["id"] != args.id] + [entry]
    index["iterations"].sort(key=lambda i: (i["date"], i.get("pr") or 0))

    for n in names:
        body = (d / n).read_bytes()
        ctype = TYPES.get(pathlib.Path(n).suffix, mimetypes.guess_type(n)[0] or "application/octet-stream")
        print(f"{'would put' if args.dry_run else 'put'} {folder}{n} ({len(body):,} bytes, {ctype})")
        if not args.dry_run:
            request("PUT", folder + n, body, ctype)
    body = json.dumps(index, indent=1).encode()
    print(f"{'would put' if args.dry_run else 'put'} {index_key} ({len(index['iterations'])} builds)")
    if not args.dry_run:
        request("PUT", index_key, body, "application/json")


if __name__ == "__main__":
    main()
