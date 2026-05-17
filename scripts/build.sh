#!/usr/bin/env bash
# Build everything: tests, fixtures, WASM bundle. Idempotent.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> cargo test"
cargo test --workspace

echo "==> regenerating EC fixtures (if upstream is available)"
EC_SRC="${EC_PARCEL_DATA:-/tmp/jason-edelman.org/eastside-commons/ec-parcel-data.js}"
if [ -f "$EC_SRC" ]; then
  node scripts/convert-ec-data.js "$EC_SRC" data/eastside-baseline.json data/eastside-proposal.json
  cp data/eastside-baseline.json data/eastside-proposal.json public/data/
else
  echo "    (skipping; $EC_SRC not found. Set EC_PARCEL_DATA env var to override.)"
fi

echo "==> building WASM bundle"
wasm-pack build crates/street-smarts-web \
    --target web \
    --release \
    --out-dir ../../public/pkg

# wasm-pack drops a .gitignore inside the output dir that excludes everything;
# we want the WASM committed/deployed, so remove it.
rm -f public/pkg/.gitignore

echo "==> bundle size"
ls -lh public/pkg/*.wasm

echo "==> done. Serve public/ locally with any static server, or run \`wrangler deploy\`."
