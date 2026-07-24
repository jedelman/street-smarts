#!/usr/bin/env bash
set -euo pipefail

# Build street-smarts-godot GDExtension library for Linux/macOS
MODE="${1:-release}"

echo "Building street-smarts-godot crate ($MODE)..."

if [ "$MODE" = "release" ]; then
    cargo build --package street-smarts-godot --release
    LIB_DIR="target/release"
else
    cargo build --package street-smarts-godot
    LIB_DIR="target/debug"
fi

mkdir -p godot/bin

if [ -f "$LIB_DIR/libstreet_smarts_godot.so" ]; then
    cp "$LIB_DIR/libstreet_smarts_godot.so" godot/bin/
    echo "Staged libstreet_smarts_godot.so -> godot/bin/"
fi

if [ -f "$LIB_DIR/libstreet_smarts_godot.dylib" ]; then
    cp "$LIB_DIR/libstreet_smarts_godot.dylib" godot/bin/
    echo "Staged libstreet_smarts_godot.dylib -> godot/bin/"
fi
