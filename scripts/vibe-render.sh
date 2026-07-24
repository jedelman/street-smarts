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
# clean_baseline plus cluster_interior:
#   - clean_baseline: eastside-baseline.json, parcel MILITARY_CIRCLE_ASSEMBLED
#     (the real Military Circle site, Norfolk VA -- a 97.7-acre union of
#     25 real Norfolk GIS parcels, no EDA tags; see the parcel's own `spec`
#     field for the real tax-parcel ids it was assembled from)
#   - cluster_interior: ONE real building cluster extracted from
#     clean_baseline's own output (tools/vibe-render/extract_cluster.py,
#     60m radius around the densest real anchor -- no --anchor given, so
#     which building that is can shift if the pipeline or fixture changes;
#     not pinned to a specific id on purpose), rendered with render.py's
#     `--interiors` flag: real p127_intimacy_gradient InteriorCell ground-
#     floor partition walls, visible through a translucent exterior shell.
#     See render.py's own `interior_partition_solids` docstring for what
#     this does and deliberately doesn't show (no stairs, furniture, room
#     labels, ceilings, or upper floors -- ground floor only, one story).
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
# just not part of the default gallery/CI render for now. Bring all six
# back for one run with:
#   VIBE_RENDER_SCENARIOS="clean_baseline cluster_interior barrio_mallcore mallcore_seeding_stratified mallcore_seeding_fieldguided" \
#     scripts/vibe-render.sh
#
# Output goes to $OUT_DIR (default: target/vibe-render/): the intermediate
# pipeline JSON, the rendered PNG/SVG files, and a per-scenario .glb (real
# 3D model, drop straight into any glTF viewer -- see render.py's own
# docstring). The animated-lineage SVG (street-smarts-ledger/examples/
# dump_lineage_animation.rs) used to be generated and embedded here too;
# dropped from the public gallery as a stale, hand-captioned section that
# never got updated as the real pipeline order changed (the example itself
# is still in the tree if it's wanted again).
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
SCENARIOS="${VIBE_RENDER_SCENARIOS:-clean_baseline cluster_interior}"
has_scenario() { case " $SCENARIOS " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }

# Wipe $OUT_DIR before generating, not just mkdir -p it -- CI (deploy.yml)
# caches the whole `target/` directory for compile speed, and $OUT_DIR's
# default (target/vibe-render) lives inside it, so a stale scenario's
# files from a PREVIOUS run (before VIBE_RENDER_SCENARIOS existed, or
# before a scenario was archived) would otherwise survive across CI runs
# untouched by this run, get swept up by the blanket `cp` below, and ship
# to production alongside the real output -- confirmed happening for
# real: barrio_mallcore.glb kept serving 200 from a deployed preview
# whose CI run never generated it, after archiving that scenario.
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

echo "==> building dump_pipeline + dump_pipeline_seeding + dump_pipeline_chain examples"
cargo build --release -p street-smarts-patterns --example dump_pipeline --example dump_pipeline_seeding --example dump_pipeline_chain
DUMP_BIN="target/release/examples/dump_pipeline"
DUMP_SEEDING_BIN="target/release/examples/dump_pipeline_seeding"
DUMP_CHAIN_BIN="target/release/examples/dump_pipeline_chain"

echo "==> running corrected pipeline: clean_baseline"
"$DUMP_BIN" data/eastside-baseline.json MILITARY_CIRCLE_ASSEMBLED "$SEED" "$OUT_DIR/clean_baseline.json"

# Substitute the real, current pipeline stage order (language_graph::LANGUAGE,
# the same table validate_order checks against the real traced pipeline
# execution) into public/index.html's gallery caption, between the
# PIPELINE_CHAIN:START/END markers -- so that caption structurally cannot
# drift from the real pipeline the way it silently did before (missing
# P124/P117/P197 even before P118/P119/P160 were added). Plain python3, not
# the render.py venv -- this only needs stdlib re, and runs before the venv
# is set up below.
echo "==> substituting real pipeline stage chain into public/index.html"
CHAIN="$("$DUMP_CHAIN_BIN")"
python3 - "$CHAIN" <<'PYEOF'
import re
import sys

chain = sys.argv[1]
path = "public/index.html"
with open(path, "r", encoding="utf-8") as f:
    html = f.read()
new_html, n = re.subn(
    r"<!-- PIPELINE_CHAIN:START -->.*?<!-- PIPELINE_CHAIN:END -->",
    lambda _m: f"<!-- PIPELINE_CHAIN:START -->{chain}<!-- PIPELINE_CHAIN:END -->",
    html,
    flags=re.DOTALL,
)
if n != 1:
    raise SystemExit(f"expected exactly 1 PIPELINE_CHAIN marker pair in {path}, found {n}")
with open(path, "w", encoding="utf-8") as f:
    f.write(new_html)
PYEOF

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

if has_scenario cluster_interior; then
  echo "==> extracting cluster: cluster_interior (from clean_baseline, 60m real radius around the densest real anchor)"
  "$VENV_DIR/bin/python" tools/vibe-render/extract_cluster.py \
    "$OUT_DIR/clean_baseline.json" "$OUT_DIR/cluster_interior.json" --radius-m 60
fi

for scenario in clean_baseline cluster_interior barrio_mallcore mallcore_seeding_stratified mallcore_seeding_fieldguided; do
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
  elif [ "$scenario" = "cluster_interior" ]; then
    # `--interiors`: real p127_intimacy_gradient InteriorCell partition
    # walls, only affordable geometry-budget-wise at single-cluster scale
    # (see render.py's own export_glb docstring for the whole-site
    # opening-geometry budget math this doesn't attempt to repeat).
    "$VENV_DIR/bin/python" tools/vibe-render/render.py "$OUT_DIR/$scenario.json" "$OUT_DIR/$scenario" --interiors
  else
    "$VENV_DIR/bin/python" tools/vibe-render/render.py "$OUT_DIR/$scenario.json" "$OUT_DIR/$scenario"
  fi
done

echo "==> done. Renders in $OUT_DIR/"
ls "$OUT_DIR"/*.png "$OUT_DIR"/*.svg "$OUT_DIR"/*.glb 2>/dev/null

if [ -n "$PUBLISH_DIR" ]; then
  echo "==> publishing every render.py artifact (png/svg/glb) to $PUBLISH_DIR"
  # Same staleness reasoning as $OUT_DIR above -- a local re-run against
  # an existing $PUBLISH_DIR with a smaller $VIBE_RENDER_SCENARIOS than a
  # prior run used would otherwise leave that prior run's now-archived
  # files sitting there untouched.
  rm -rf "$PUBLISH_DIR"
  mkdir -p "$PUBLISH_DIR"
  # Blanket copy, not a per-file allowlist: every artifact type render.py
  # produces (isometric PNG, floor-plan SVGs, .glb) for
  # every scenario, under its own real filename -- so a future new output
  # type is published automatically without touching this script again.
  # index.html links against these exact filenames.
  cp "$OUT_DIR"/*.png "$OUT_DIR"/*.svg "$OUT_DIR"/*.glb "$PUBLISH_DIR/" 2>/dev/null
  # NOT part of the blanket copy above -- real *.json in $OUT_DIR also
  # includes each scenario's own full pipeline dump (cluster_interior.json,
  # clean_baseline.json, multi-MB), which was never meant to be a public
  # asset. Only *_interior_views.json (render.py's own real <model-viewer>
  # camera-target/camera-orbit numbers -- see interior_camera_views' own
  # docstring) is a real, small, intentionally public artifact.
  cp "$OUT_DIR"/*_interior_views.json "$PUBLISH_DIR/" 2>/dev/null
  ls "$PUBLISH_DIR"
fi
