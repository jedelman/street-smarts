#!/usr/bin/env bash
set -euo pipefail

# Builds the Android GDExtension library and exports a signed debug APK,
# named with the current commit's short SHA (e.g.
# street-smarts-debug-822e969.apk) so successive test builds sent to a
# device don't all collide under one filename -- there was no way to tell
# which commit an installed APK actually came from before this existed.
#
# Assumes the same machine-specific setup NIXOS_DEV_ENVIRONMENT.md
# documents: Android SDK/NDK path via .cargo/config.toml (not this
# script), Godot editor settings/export templates/keystore staged under
# $EXPORT_HOME (see below), and the "Android" export preset already
# defined in godot/export_presets.cfg.

cd "$(dirname "$0")/.."

# Godot's export templates/SDK path/keystore are looked up under $HOME --
# on a machine where the real setup lives under a different user's home
# than the shell's own $HOME (a real footgun this session hit: Godot
# silently created an empty, unconfigured $HOME/.config/godot and failed
# with "No export template found" / "valid Android SDK path required"),
# override EXPORT_HOME to point at wherever that setup actually is.
EXPORT_HOME="${EXPORT_HOME:-$HOME}"
GODOT_BIN="${GODOT_BIN:-godot4}"

SHA="$(git rev-parse --short HEAD)"
DIRTY=""
if [ -n "$(git status --porcelain)" ]; then
    DIRTY="-dirty"
fi
OUT_NAME="street-smarts-debug-${SHA}${DIRTY}.apk"

echo "Building street-smarts-godot for Android (release, aarch64)..."
cargo build --release -p street-smarts-godot --target aarch64-linux-android

mkdir -p godot/bin/android
cp target/aarch64-linux-android/release/libstreet_smarts_godot.so godot/bin/android/

mkdir -p godot/build
echo "Exporting $OUT_NAME..."
(cd godot && HOME="$EXPORT_HOME" "$GODOT_BIN" --headless --export-debug "Android" "build/$OUT_NAME")

echo "Done: godot/build/$OUT_NAME"
