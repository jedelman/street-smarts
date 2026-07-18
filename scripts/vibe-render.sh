#!/usr/bin/env bash
# 3D "vibe test": run the corrected pattern pipeline on real fixture data,
# then extrude the result into massing solids and render plan/elevation/
# isometric views. A gut check on scale and density, not a real
# architectural rendering -- see tools/vibe-render/render.py for caveats.
#
# Runs three scenarios:
#   - clean_baseline: eastside-baseline.json, parcel MILITARY_CIRCLE_ASSEMBLED
#     (the real Military Circle site, Norfolk VA -- a 97.7-acre union of
#     25 real Norfolk GIS parcels, no EDA tags; see the parcel's own `spec`
#     field for the real tax-parcel ids it was assembled from)
#   - barrio_mallcore: eastside-proposal.json, parcel 13279568 (MALL_CORE,
#     27.8 acres, the fragmentation stress case -- see task #7)
#   - mallcore_seeding: MALL_CORE again, rendered twice -- once with P37's
#     default Stratified seeding, once with the FieldGuided prototype
#     (seeding_mode=1.0) -- so the two can be compared side by side. See
#     crates/street-smarts-patterns/src/field.rs and
#     examples/dump_pipeline_seeding.rs.
#
# Output goes to $OUT_DIR (default: target/vibe-render/): the intermediate
# pipeline JSON, the rendered PNG/SVG files, and a per-scenario .glb (real
# 3D model, drop straight into any glTF viewer -- see render.py's own
# docstring).
#
# If $PUBLISH_DIR is set, every render (isometric PNGs, plan/elevation/
# floor-plan SVGs, .glb) is also copied there under fixed filenames, ready
# to ship as static site assets -- the gallery embedding them lives at the
# TOP of public/index.html (section#vibe-gallery), not a separate page.
set -euo pipefail

cd "$(dirname "$0")/.."

OUT_DIR="${OUT_DIR:-target/vibe-render}"
VENV_DIR="${VENV_DIR:-tools/vibe-render/.venv}"
SEED="${VIBE_RENDER_SEED:-42}"
PUBLISH_DIR="${PUBLISH_DIR:-}"

mkdir -p "$OUT_DIR"

echo "==> building dump_pipeline + dump_pipeline_seeding examples"
cargo build --release -p street-smarts-patterns --example dump_pipeline --example dump_pipeline_seeding
DUMP_BIN="target/release/examples/dump_pipeline"
DUMP_SEEDING_BIN="target/release/examples/dump_pipeline_seeding"

echo "==> running corrected pipeline: clean_baseline"
"$DUMP_BIN" data/eastside-baseline.json MILITARY_CIRCLE_ASSEMBLED "$SEED" "$OUT_DIR/clean_baseline.json"

echo "==> running corrected pipeline: barrio_mallcore"
"$DUMP_BIN" data/eastside-proposal.json 13279568 "$SEED" "$OUT_DIR/barrio_mallcore.json"

echo "==> running corrected pipeline: mallcore_seeding (stratified vs field-guided)"
"$DUMP_SEEDING_BIN" data/eastside-proposal.json 13279568 "$SEED" "$OUT_DIR/mallcore_seeding"

echo "==> preparing Python render environment"
if [ ! -d "$VENV_DIR" ]; then
  python3 -m venv "$VENV_DIR"
fi
"$VENV_DIR/bin/pip" install -q -r tools/vibe-render/requirements.txt

for scenario in clean_baseline barrio_mallcore mallcore_seeding_stratified mallcore_seeding_fieldguided; do
  echo "==> rendering: $scenario"
  "$VENV_DIR/bin/python" tools/vibe-render/render.py "$OUT_DIR/$scenario.json" "$OUT_DIR/$scenario"
done

echo "==> done. Renders in $OUT_DIR/"
ls "$OUT_DIR"/*.png "$OUT_DIR"/*.svg "$OUT_DIR"/*.glb 2>/dev/null

if [ -n "$PUBLISH_DIR" ]; then
  echo "==> publishing every render.py artifact (png/svg/glb) to $PUBLISH_DIR"
  mkdir -p "$PUBLISH_DIR"
  # Blanket copy, not a per-file allowlist: every artifact type render.py
  # produces (isometric PNG, plan/elevation/floor-plan SVGs, .glb) for
  # every scenario, under its own real filename -- so a future new output
  # type is published automatically without touching this script again.
  # index.html links against these exact filenames.
  cp "$OUT_DIR"/*.png "$OUT_DIR"/*.svg "$OUT_DIR"/*.glb "$PUBLISH_DIR/" 2>/dev/null
  ls "$PUBLISH_DIR"
fi
