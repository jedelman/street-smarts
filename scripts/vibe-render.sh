#!/usr/bin/env bash
# 3D "vibe test": run the corrected pattern pipeline on real fixture data,
# then extrude the result into massing solids and render floor-plan and
# isometric views, plus a .glb for interactive viewing in the browser
# (<model-viewer> on the public gallery). A gut check on scale and
# density, not a real architectural rendering -- see
# tools/vibe-render/render.py for caveats. Used to also render plan/
# elevation SVGs via a slow OpenCascade hidden-line-removal export
# (~58% of render.py's time); dropped once the interactive .glb covered
# the same "look at the massing from outside" need for a fraction of the
# cost.
#
# Runs whichever scenarios $VIBE_RENDER_SCENARIOS names -- defaults to
# just clean_baseline (the one real, geolocated site):
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
# barrio_mallcore/mallcore_seeding are archived, not deleted, as of the
# commit that added this env var -- real fixture data and real pipeline
# code, still fully valid (nothing about the pipeline itself changed),
# just not part of the default gallery/CI render for now. Bring all four
# back for one run with:
#   VIBE_RENDER_SCENARIOS="clean_baseline barrio_mallcore mallcore_seeding_stratified mallcore_seeding_fieldguided" \
#     scripts/vibe-render.sh
#
# Output goes to $OUT_DIR (default: target/vibe-render/): the intermediate
# pipeline JSON, the rendered PNG/SVG files, a per-scenario .glb (real
# 3D model, drop straight into any glTF viewer -- see render.py's own
# docstring), and clean_baseline_lineage.svg -- a self-contained animated
# SVG (SMIL, no JS) of the same P37 -> per-block P95 -> P108 commit chain
# unfolding in real geometry, from
# street-smarts-ledger/examples/dump_lineage_animation.rs. Deliberately
# the NON-satellite variant (dump_lineage_map.rs's combined satellite+graph
# version is ~30x heavier, a base64-embedded raster tile) -- this repo
# already gates its WASM bundle to a 300KB gzip budget, so a multi-MB
# inline SVG isn't something to ship on the public page by default.
#
# If $PUBLISH_DIR is set, every render (isometric PNGs, floor-plan SVGs,
# .glb) is also copied there under fixed filenames, ready
# to ship as static site assets -- the gallery embedding them lives at the
# TOP of public/index.html (section#vibe-gallery), not a separate page.
set -euo pipefail

cd "$(dirname "$0")/.."

OUT_DIR="${OUT_DIR:-target/vibe-render}"
VENV_DIR="${VENV_DIR:-tools/vibe-render/.venv}"
SEED="${VIBE_RENDER_SEED:-42}"
PUBLISH_DIR="${PUBLISH_DIR:-}"
SCENARIOS="${VIBE_RENDER_SCENARIOS:-clean_baseline}"
has_scenario() { case " $SCENARIOS " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }

mkdir -p "$OUT_DIR"

echo "==> building dump_pipeline + dump_pipeline_seeding + dump_lineage_animation examples"
cargo build --release -p street-smarts-patterns --example dump_pipeline --example dump_pipeline_seeding
cargo build --release -p street-smarts-ledger --example dump_lineage_animation
DUMP_BIN="target/release/examples/dump_pipeline"
DUMP_SEEDING_BIN="target/release/examples/dump_pipeline_seeding"
DUMP_LINEAGE_ANIMATION_BIN="target/release/examples/dump_lineage_animation"

echo "==> running corrected pipeline: clean_baseline"
"$DUMP_BIN" data/eastside-baseline.json MILITARY_CIRCLE_ASSEMBLED "$SEED" "$OUT_DIR/clean_baseline.json"

echo "==> rendering: clean_baseline lineage animation (real geometry, no satellite -- see dump_lineage_animation.rs)"
"$DUMP_LINEAGE_ANIMATION_BIN" data/eastside-baseline.json MILITARY_CIRCLE_ASSEMBLED "$SEED" "$OUT_DIR/clean_baseline_lineage.svg" 1.2

if has_scenario barrio_mallcore; then
  echo "==> running corrected pipeline: barrio_mallcore"
  "$DUMP_BIN" data/eastside-proposal.json 13279568 "$SEED" "$OUT_DIR/barrio_mallcore.json"
fi

if has_scenario mallcore_seeding_stratified || has_scenario mallcore_seeding_fieldguided; then
  echo "==> running corrected pipeline: mallcore_seeding (stratified vs field-guided)"
  "$DUMP_SEEDING_BIN" data/eastside-proposal.json 13279568 "$SEED" "$OUT_DIR/mallcore_seeding"
fi

echo "==> preparing Python render environment"
if [ ! -d "$VENV_DIR" ]; then
  python3 -m venv "$VENV_DIR"
fi
"$VENV_DIR/bin/pip" install -q -r tools/vibe-render/requirements.txt

for scenario in clean_baseline barrio_mallcore mallcore_seeding_stratified mallcore_seeding_fieldguided; do
  has_scenario "$scenario" || continue
  echo "==> rendering: $scenario"
  if [ "$scenario" = "clean_baseline" ]; then
    # Real surrounding-building massing (Overture Maps, pre-filtered to
    # exclude our own site -- see the file's own _provenance field) --
    # only for clean_baseline, the scenario with real, geolocated site
    # data. The other scenarios' fixtures aren't real single addresses in
    # the same way, so there's no honest "surrounding context" to fetch.
    "$VENV_DIR/bin/python" tools/vibe-render/render.py "$OUT_DIR/$scenario.json" "$OUT_DIR/$scenario" \
      data/military-circle-context-buildings.geojson
  else
    "$VENV_DIR/bin/python" tools/vibe-render/render.py "$OUT_DIR/$scenario.json" "$OUT_DIR/$scenario"
  fi
done

echo "==> done. Renders in $OUT_DIR/"
ls "$OUT_DIR"/*.png "$OUT_DIR"/*.svg "$OUT_DIR"/*.glb 2>/dev/null

if [ -n "$PUBLISH_DIR" ]; then
  echo "==> publishing every render.py artifact (png/svg/glb) to $PUBLISH_DIR"
  mkdir -p "$PUBLISH_DIR"
  # Blanket copy, not a per-file allowlist: every artifact type render.py
  # produces (isometric PNG, floor-plan SVGs, .glb) for
  # every scenario, under its own real filename -- so a future new output
  # type is published automatically without touching this script again.
  # index.html links against these exact filenames.
  cp "$OUT_DIR"/*.png "$OUT_DIR"/*.svg "$OUT_DIR"/*.glb "$PUBLISH_DIR/" 2>/dev/null
  ls "$PUBLISH_DIR"
fi
