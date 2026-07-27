# Moving development onto the NixOS box (2GB VRAM) — plan + first-day checklist

**Status:** Not yet tried. Written from this session's own measured numbers so day one
on the new box is a real before/after comparison, not a vibe check.

**A `flake.nix` already existed in this repo** (from an earlier session, not this
one) — a CUDA 12.6 shell for `ferrotorch` on "Maxwell (sm_50)". That's real,
independent corroboration of "2GB VRAM": Maxwell-generation cards top out around
there (GTX 750 Ti / 950 / Quadro K2200-class), so this is very likely the same box.
I extended that file rather than replacing it: `nix develop` (default) is still the
existing CUDA/ferrotorch shell, untouched; `nix develop .#godot` is a new shell with
the Rust/Android/Godot toolchain this migration needs. Both are declared in the same
`flake.nix` so they share the repo's actual dependency graph instead of drifting.
The `.#godot` shell is untested (no `nix` binary in this cloud sandbox to try it
against) — treat it as a documented starting point to fix, not a finished tool.

## Why this might actually help (and why it might not)

This session hit two very different kinds of slowness, and only one of them is a GPU
problem:

1. **Surface Nets extraction / pipeline runs — CPU-bound, not GPU-bound.** Meshing the
   full real 35-building site takes ~6.5-7.2s (parallelized across CPU cores already,
   see `NeighborhoodNode3D::rebuild_3d_mesh`'s own doc). A GPU does nothing for this.
   If the NixOS box has more/faster CPU cores than this container, that's a real win;
   if not, this number won't move.
2. **Offscreen visual verification — genuinely GPU-bound today, and genuinely broken.**
   This sandbox has no GPU, so every screenshot this session went through
   `LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe` — Mesa's CPU software rasterizer.
   The 9-building cluster test scene (835K triangles) rendered fine in ~10-15s wall
   time. The **full 35-building site (3.4M triangles) never finished** — I killed it
   after 5+ minutes of one llvmpipe frame. A real GPU, even a modest 2GB one, should
   render either of those in a handful of milliseconds. This is the actual case for
   the move.

Android NDK cross-compilation is also CPU/RAM-bound (a release build of
`street-smarts-godot` for `aarch64-linux-android` took ~27 minutes in this container;
Linux release ~7 minutes) — again, only helped by better CPU/RAM, not VRAM.

**Bottom line:** the move is well-justified specifically for offscreen rendering /
visual verification, which is the one thing this container structurally can't do
well. It's an open question for everything CPU-bound until we know that box's CPU.

## Baseline numbers to compare against (measured this session, this container)

| Task | Scene | Result |
|---|---|---|
| Pipeline + mesh rebuild | Full site (35 bldgs, 3.4M tris) | ~6.5-7.2s |
| Pipeline + mesh rebuild | Cluster (9 bldgs, 835K tris) | ~1.4s |
| Offscreen render (llvmpipe, 3 screenshots) | Cluster | ~10-15s wall time |
| Offscreen render (llvmpipe) | Full site | **Never finished — killed after 5+ min** |
| `cargo build --release` (Linux, street-smarts-godot) | — | ~7 min |
| `cargo build --release --target aarch64-linux-android` | — | ~27 min |

First thing to do on the NixOS box: rerun the exact same cluster-scene offscreen
render (`godot/scenes/ClusterTest.tscn`, see below) with a **real** rendering driver
instead of the llvmpipe env vars, and record the same table. If it's not dramatically
faster, something's wrong with GPU passthrough/drivers, not with the plan.

## First-day checklist

1. **Confirm the GPU is actually reachable.**
   `glxinfo | grep "OpenGL renderer"` (or `vulkaninfo --summary` if targeting Vulkan)
   must NOT say `llvmpipe`. If it does, Godot will silently fall back to software
   rendering again and none of this was worth doing — fix drivers/passthrough first.
   The existing CUDA shell's `nvidia-smi` check is a faster first signal: if that
   doesn't see the GPU either, this is a NixOS system-config problem
   (`hardware.nvidia.*`, `hardware.opengl.enable` in `/etc/nixos/configuration.nix`
   or the flake-based equivalent), not something either devShell can fix on its own
   — a devShell can only point at `/run/opengl-driver/lib`, it can't supply the
   kernel driver itself. Maxwell is old enough that it's worth double-checking the
   NixOS nvidia driver package actually still supports sm_50 before assuming a
   config bug.
2. **Bring up the Godot dev shell:** `nix develop .#godot` at the repo root (from
   `flake.nix` — `nix develop` alone gives you the existing CUDA/ferrotorch shell,
   not this one). Untested — expect to patch package names/versions. See "Known
   footguns" below for mistakes already made once in the cloud container; don't
   remake them.
3. **Re-stage the Android export assets.** Editor settings, export templates, and the
   debug keystore are NOT in git (correctly — they're machine-specific/generated).
   They'll need to be reinstalled on the new box:
   - Godot export templates for 4.3.stable (`Editor > Manage Export Templates`, or
     download `Godot_v4.3-stable_export_templates.tpz` directly).
   - Android SDK + NDK 23.2.8568313 (or update `.cargo/config.toml` / the flake to
     whatever NDK version Nix actually gives you — don't fight Nix to get this exact
     version if it's inconvenient, just update the one file that hardcodes the path).
   - A debug keystore for signing (`keytool -genkey -v -keystore debug.keystore
     -storepass android -alias androiddebugkey -keypass android -keyalg RSA
     -validity 10000` if starting fresh).
4. **Run the offscreen render benchmark** (see above) and fill in the comparison
   table before touching any real feature work — that's the actual test drive.

## Known footguns from this session (don't re-lose the time)

- **`$HOME` mismatch silently breaks Android export.** Godot's editor settings,
  export templates, and keystore all live under `$HOME/.config/godot` and
  `$HOME/.local/share/godot`. This container's shell has `$HOME=/root`, but all the
  Android export setup from earlier in the session was done under `$HOME=/home/user`
  — so every `--export-debug` call silently created a FRESH, empty `/root/.config/godot`
  and failed with "No export template found" / "valid Android SDK path required",
  even though a fully-configured one existed one directory over. Fixed by running
  Godot with `HOME=/home/user` explicitly. On the new box: pick one `$HOME`, do all
  setup under it, and don't switch users/shells partway through a session.
- **`--headless` uses the "dummy" render backend — it cannot produce a real
  screenshot.** `get_viewport().get_texture()` returns null under `--headless` even
  with `--rendering-driver opengl3` passed; the actual renderer silently becomes
  `servers/rendering/dummy`. Real screenshots need a real (or Xvfb) display and Godot
  run *without* `--headless`. On a NixOS box with an actual GPU and display, this
  should be simpler than the Xvfb dance, not harder — try a real window first.
- **A specific camera angle (yaw=90°, pitch=6°) produced a byte-for-byte frozen
  screenshot under llvmpipe**, reproducible across independent process runs, even
  though the camera's own transform was verified correct and stable for 30+ frames
  before capture. Isolated to that one angle (a known-good angle on the identical
  scene rendered correctly) — never root-caused, left as an open question. Worth a
  quick re-test on real hardware GL: if it reproduces there too, it's a real Godot/
  Godot-Rust bug worth chasing; if it only happens under llvmpipe, it's a software-
  rasterizer quirk we can stop worrying about.
- **`.cargo/config.toml`'s NDK paths are hardcoded to this container**
  (`/home/user/android-sdk/ndk/23.2.8568313/...`). Nix store paths are
  content-addressed and unpredictable ahead of time, so the flake's dev shell sets
  the same `CC_aarch64_linux_android` / `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER`
  env vars dynamically instead of relying on the checked-in `.cargo/config.toml` —
  don't expect that file to work unmodified outside this exact container.

## Versions this project currently targets (as of this session)

For reference when the Nix-provided versions inevitably differ slightly:

| Tool | Version |
|---|---|
| Godot | 4.3-stable (official build `77dcf97d8`) |
| rustc / cargo | 1.94.1 |
| Rust targets in use | `aarch64-linux-android`, `wasm32-unknown-emscripten`, `x86_64-unknown-linux-gnu` |
| Android SDK platform | android-34 |
| Android build-tools | 34.0.0 |
| Android NDK | 23.2.8568313 |
| Java | OpenJDK 21.0.10 |
| gdext (godot-rust) | 0.2.4 |

None of these are hard requirements — they're just what's proven to work together
today. If Nix makes a slightly newer Godot/NDK/JDK easy and a matching one is
painful, prefer what's easy and re-verify against the test suite
(`cargo test -p street-smarts-godot -p street-smarts-core`, 18 + 44 tests as of this
session, all should stay green) rather than fighting for exact version parity.
