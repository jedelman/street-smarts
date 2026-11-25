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
# and wasm-pack are installed here (about a minute), and it has no libGL, which
# cadquery needs, so Ubuntu's packages for it are unpacked into the home
# directory (see below). Tested from an empty home directory: 4 m 38 s end to
# end; the libGL step tested as a non-root user with the system's GL hidden.
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

# cadquery's OCP links libGL and libX11, which Cloudflare's build image lacks
# (first build: "ImportError: libGL.so.1: cannot open shared object file"), and
# builds there can't apt-get install. So fetch Ubuntu's own packages into the
# home directory without root and point the loader at them. Importing them loads
# no GPU driver; nothing here draws with OpenGL (matplotlib renders with Agg).
if ! ldconfig -p 2>/dev/null | grep -q 'libGL\.so\.1'; then
  echo "==> libGL missing: fetching Ubuntu's libgl1 and friends without root"
  apt_dir="$HOME/.cache/street-smarts-apt"
  gl_dir="$HOME/.cache/street-smarts-gl"
  mkdir -p "$apt_dir/lists/partial" "$apt_dir/cache/archives/partial" "$apt_dir/debs" "$gl_dir"
  apt_opts=(-o Debug::NoLocking=1 -o APT::Sandbox::User="$(id -un)"
            -o Dir::State::Lists="$apt_dir/lists" -o Dir::Cache="$apt_dir/cache")
  apt-get "${apt_opts[@]}" -qq update
  (cd "$apt_dir/debs" && apt-get "${apt_opts[@]}" -qq download \
    libgl1 libglx0 libglvnd0 libx11-6 libx11-data libxcb1 libxau6 libxdmcp6 libbsd0 libmd0)
  for deb in "$apt_dir"/debs/*.deb; do dpkg-deb -x "$deb" "$gl_dir"; done
  export LD_LIBRARY_PATH="$gl_dir/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi

echo "==> rendering the vibe-render gallery"
PUBLISH_DIR=public/vibe-render bash scripts/vibe-render.sh

echo "==> checking the GLBs are packed"
tools/vibe-render/.venv/bin/python3 tools/vibe-render/check_packed.py public/vibe-render/*.glb
