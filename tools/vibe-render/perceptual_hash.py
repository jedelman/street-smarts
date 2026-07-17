#!/usr/bin/env python3
"""Perceptual-hash regression check for vibe-render's PNG outputs --
HARDENING_SPEC.md §4.

Determinism confirmed before this was built, not assumed: render.py's
isometric PNG output is byte-identical across two runs on identical input
(verified separately, not by this script). The SVG and .glb outputs are
NOT currently byte-deterministic (a matplotlib-embedded creation
timestamp + randomized clip-path ID in the SVG case, a UUID1 node name in
the .glb case) -- both confirmed cosmetic, not geometric, but that means
this script only covers PNGs for now. Extending it to SVG/.glb needs
canonicalization first (strip the timestamp/ID fields before hashing),
not just pointing this same logic at them.

Uses a plain average-hash (aHash): downscale to 8x8 grayscale, threshold
against the mean, pack into a 64-bit int. Cheap, dependency-light (only
needs PIL, already a transitive dependency via matplotlib), and exactly
matched to what this check needs to catch -- a pattern silently stopped
generating buildings, a color/scale regression, a degenerate render --
not fine-grained pixel-perfect equality (that's what the underlying PNG
determinism already gives for free; this hash is for the "did the
RENDERED PICTURE change" question a human reviewer actually cares about).
"""
import json
import sys
from pathlib import Path

from PIL import Image


def ahash(png_path: str, hash_size: int = 8) -> str:
    img = Image.open(png_path).convert("L").resize((hash_size, hash_size), Image.LANCZOS)
    pixels = list(img.getdata())
    avg = sum(pixels) / len(pixels)
    bits = "".join("1" if p >= avg else "0" for p in pixels)
    return f"{int(bits, 2):0{hash_size * hash_size // 4}x}"


def hamming_distance(hash_a: str, hash_b: str) -> int:
    return bin(int(hash_a, 16) ^ int(hash_b, 16)).count("1")


def cmd_hash(args):
    print(ahash(args[0]))


def cmd_check(args):
    """check <baseline.json> <scenario_dir>
    baseline.json: {"scenario_name.png": "hexhash", ...}
    Exits 1 and prints every scenario whose hash drifted beyond TOLERANCE.
    """
    TOLERANCE_BITS = 4  # out of 64 -- small, deliberate tolerance for
    # sub-pixel antialiasing jitter across machines, not a "close enough"
    # excuse for a real visual change.
    baseline_path, scenario_dir = args
    baseline = json.loads(Path(baseline_path).read_text())
    failures = []
    for name, expected_hash in baseline.items():
        png_path = Path(scenario_dir) / name
        if not png_path.exists():
            failures.append(f"{name}: MISSING (expected file not produced)")
            continue
        actual_hash = ahash(str(png_path))
        dist = hamming_distance(expected_hash, actual_hash)
        if dist > TOLERANCE_BITS:
            failures.append(f"{name}: hash drift {dist} bits (expected {expected_hash}, got {actual_hash})")
    if failures:
        print("VISUAL REGRESSION CHECK FAILED:")
        for f in failures:
            print(f"  {f}")
        sys.exit(1)
    print(f"OK: {len(baseline)} scenario(s) match within {TOLERANCE_BITS}-bit tolerance.")


def cmd_update_baseline(args):
    """update-baseline <baseline.json> <scenario_dir> <png_name> [<png_name> ...]
    Regenerates the baseline hash for named PNGs -- the explicit,
    scripted re-baseline path (matches cargo-insta / jest snapshot
    workflows), used when a change intentionally alters a render.
    """
    baseline_path, scenario_dir, *names = args
    baseline_file = Path(baseline_path)
    baseline = json.loads(baseline_file.read_text()) if baseline_file.exists() else {}
    for name in names:
        png_path = Path(scenario_dir) / name
        baseline[name] = ahash(str(png_path))
        print(f"updated {name}: {baseline[name]}")
    baseline_file.write_text(json.dumps(baseline, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("usage: perceptual_hash.py <hash|check|update-baseline> ...", file=sys.stderr)
        sys.exit(2)
    cmd, rest = sys.argv[1], sys.argv[2:]
    {"hash": cmd_hash, "check": cmd_check, "update-baseline": cmd_update_baseline}[cmd](rest)
