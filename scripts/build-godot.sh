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

# Android (arm64) -- opt-in, requires ANDROID_NDK_HOME and the
# aarch64-linux-android rustup target. Not part of the default Linux/macOS
# build above because it needs its own linker env vars.
if [ -n "${ANDROID_NDK_HOME:-}" ]; then
    echo "Building street-smarts-godot crate for Android (aarch64, $MODE)..."
    TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64"
    export CLANG_PATH="$TOOLCHAIN/bin/clang"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$TOOLCHAIN/bin/aarch64-linux-android24-clang"
    export CC_aarch64_linux_android="$TOOLCHAIN/bin/aarch64-linux-android24-clang"
    export AR_aarch64_linux_android="$TOOLCHAIN/bin/llvm-ar"

    if [ "$MODE" = "release" ]; then
        cargo build --package street-smarts-godot --target aarch64-linux-android --release
        ANDROID_LIB_DIR="target/aarch64-linux-android/release"
    else
        cargo build --package street-smarts-godot --target aarch64-linux-android
        ANDROID_LIB_DIR="target/aarch64-linux-android/debug"
    fi

    mkdir -p godot/bin/android
    cp "$ANDROID_LIB_DIR/libstreet_smarts_godot.so" godot/bin/android/
    echo "Staged libstreet_smarts_godot.so -> godot/bin/android/"
fi
