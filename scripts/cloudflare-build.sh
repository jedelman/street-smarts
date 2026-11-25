#!/usr/bin/env bash
# The build Cloudflare Workers Builds runs before `npx wrangler deploy`.
# Dashboard: Workers & Pages > street-smarts > Settings > Build:
#   build command   bash scripts/cloudflare-build.sh
#   deploy command  npx wrangler deploy           (the default)
#   Python          3.11, from .python-version     (cadquery's wheels)
#
# It fills public/ the way the deploy needs it: the WASM app in public/pkg,
# the vibe-render GLBs and renders in public/vibe-render, and the pipeline's
# stage chain written into public/index.html. The tests and the other gates
# run in GitHub Actions (.github/workflows/ci.yml) on every PR; this only
# refuses to deploy unpacked GLBs.
#
# Cloudflare's build image has Node and Python but not Rust, so the toolchain
# and wasm-pack are installed here (about a minute). Tested from an empty home
# directory: 4 m 38 s end to end.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v cargo >/dev/null 2>&1; then
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --target wasm32-unknown-unknown
fi
# shellcheck disable=SC1091
. "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown >/dev/null

if ! command -v wasm-pack >/dev/null 2>&1; then
  curl -sSf https://rustwasm.github.io/wasm-pack/installer/init.sh | sh
fi

echo "==> building the WASM app"
wasm-pack build crates/street-smarts-web --target web --release --out-dir ../../public/pkg
rm -f public/pkg/.gitignore

echo "==> rendering the vibe-render gallery"
PUBLISH_DIR=public/vibe-render bash scripts/vibe-render.sh

echo "==> checking the GLBs are packed"
tools/vibe-render/.venv/bin/python3 tools/vibe-render/check_packed.py public/vibe-render/*.glb
