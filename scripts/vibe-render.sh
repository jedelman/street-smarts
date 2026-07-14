#!/usr/bin/env bash
# 3D "vibe test": run the corrected pattern pipeline on real fixture data,
# then extrude the result into massing solids and render plan/elevation/
# isometric views. A gut check on scale and density, not a real
# architectural rendering -- see tools/vibe-render/render.py for caveats.
#
# Runs two scenarios that have earned their keep as regression cases:
#   - clean_baseline: eastside-baseline.json, parcel 00001129 (47.7-acre
#     pre-redevelopment mega-parcel, no EDA tags)
#   - barrio_mallcore: eastside-proposal.json, parcel 13279568 (MALL_CORE,
#     27.8 acres, the fragmentation stress case -- see task #7)
#
# Output goes to $OUT_DIR (default: target/vibe-render/), both the
# intermediate pipeline JSON and the rendered PNG/SVG files.
set -euo pipefail

cd "$(dirname "$0")/.."

OUT_DIR="${OUT_DIR:-target/vibe-render}"
VENV_DIR="${VENV_DIR:-tools/vibe-render/.venv}"
SEED="${VIBE_RENDER_SEED:-42}"

mkdir -p "$OUT_DIR"

echo "==> building dump_pipeline example"
cargo build --release -p street-smarts-patterns --example dump_pipeline
DUMP_BIN="target/release/examples/dump_pipeline"

echo "==> running corrected pipeline: clean_baseline"
"$DUMP_BIN" data/eastside-baseline.json 00001129 "$SEED" "$OUT_DIR/clean_baseline.json"

echo "==> running corrected pipeline: barrio_mallcore"
"$DUMP_BIN" data/eastside-proposal.json 13279568 "$SEED" "$OUT_DIR/barrio_mallcore.json"

echo "==> preparing Python render environment"
if [ ! -d "$VENV_DIR" ]; then
  python3 -m venv "$VENV_DIR"
fi
"$VENV_DIR/bin/pip" install -q -r tools/vibe-render/requirements.txt

echo "==> rendering: clean_baseline"
"$VENV_DIR/bin/python" tools/vibe-render/render.py "$OUT_DIR/clean_baseline.json" "$OUT_DIR/clean_baseline"

echo "==> rendering: barrio_mallcore"
"$VENV_DIR/bin/python" tools/vibe-render/render.py "$OUT_DIR/barrio_mallcore.json" "$OUT_DIR/barrio_mallcore"

echo "==> done. Renders in $OUT_DIR/"
ls "$OUT_DIR"/*.png "$OUT_DIR"/*.svg 2>/dev/null
