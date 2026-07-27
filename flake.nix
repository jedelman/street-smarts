{
  description = "street-smarts dev shells — CUDA 12.6 for ferrotorch (default), Godot 4.3 + Android NDK for the client migration (.#godot)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
    android-nixpkgs = {
      url = "github:tadfisher/android-nixpkgs";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, android-nixpkgs }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
      config.allowUnfree = true;
      config.cudaSupport = true;
    };
    cudaPackages = pkgs.cudaPackages_12_6;

    # For the Godot shell -- separate pkgs import so rust-overlay doesn't need
    # cudaSupport/allowUnfree, and so a `nix develop .#godot` that never touches
    # CUDA doesn't need to evaluate/fetch it.
    godotPkgs = import nixpkgs {
      inherit system;
      overlays = [ rust-overlay.overlays.default ];
    };

    # Pinned to what this project verified working in a cloud container this
    # session (see NIXOS_DEV_ENVIRONMENT.md's version table). Bump deliberately,
    # not by accident -- re-run the full test suite after any change here.
    rustToolchain = godotPkgs.rust-bin.stable."1.94.1".default.override {
      extensions = [ "rust-src" ];
      targets = [ "aarch64-linux-android" "x86_64-unknown-linux-gnu" ];
    };

    androidSdk = android-nixpkgs.sdk.${system} (sdkPkgs: with sdkPkgs; [
      cmdline-tools-latest
      platform-tools
      build-tools-34-0-0
      platforms-android-34
      ndk-23-2-8568313
    ]);

    # NDK_HOME under the Nix store is a content-addressed path, unlike the
    # cloud container's hardcoded /home/user/android-sdk/... in
    # .cargo/config.toml -- resolved here at shell-entry time instead, since
    # it won't survive the move to a different store path. See
    # NIXOS_DEV_ENVIRONMENT.md's "Known footguns" section.
    ndkBin = "${androidSdk}/share/android-sdk/ndk/23.2.8568313/toolchains/llvm/prebuilt/linux-x86_64/bin";
  in {
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = [
        # CUDA 12.6 toolkit
        cudaPackages.cuda_cudart
        cudaPackages.cuda_nvcc
        # These packages split .so into a .lib output
        cudaPackages.libcublas.lib
        cudaPackages.libcusolver.lib
        cudaPackages.libcufft.lib
        cudaPackages.libcusparse.lib

        # Geo dependencies (gdal crate)
        pkgs.gdal
        pkgs.pkg-config
        pkgs.cmake

        # bindgen needs libclang
        pkgs.llvmPackages.libclang.lib
        pkgs.clang
      ];

      shellHook = ''
        export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib"
        export CUDA_PATH="${cudaPackages.cuda_cudart}"
        export CUDARC_CUDA_VERSION=12060
        export FERROTORCH_PTX_ARCH=sm_50
        export FERROTORCH_NVRTC_ARCH=compute_50
        export LD_LIBRARY_PATH="/run/opengl-driver/lib:${cudaPackages.cuda_cudart}/lib:${cudaPackages.libcublas.lib}/lib:${cudaPackages.libcusolver.lib}/lib:${cudaPackages.libcufft.lib}/lib:${cudaPackages.libcusparse.lib}/lib:$LD_LIBRARY_PATH"
        echo "street-smarts dev shell — CUDA 12.6, Maxwell (sm_50)"
        echo "  CUDARC_CUDA_VERSION=$CUDARC_CUDA_VERSION"
        echo "  GPU: $(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null || echo 'nvidia-smi not found')"
      '';
    };

    # `nix develop .#godot` -- the Godot/Rust/Android toolchain for the
    # Godot-client migration (see GODOT_PORT_SPEC.md, NIXOS_DEV_ENVIRONMENT.md).
    # UNTESTED as of writing: no `nix` binary was available in the cloud
    # container this was written in. Treat package names/android-nixpkgs usage
    # as a documented best guess, not a working artifact -- first thing to do
    # on the real box is `nix develop .#godot` and fix whatever breaks.
    #
    # Deliberately a SEPARATE shell from `.default` rather than merged into it:
    # this project's Godot work doesn't need CUDA, and pulling CUDA packages
    # just to run Godot would be a real, avoidable cost on every shell entry.
    devShells.${system}.godot = godotPkgs.mkShell {
      buildInputs = [
        rustToolchain
        androidSdk
        godotPkgs.jdk21
        godotPkgs.godot_4
        godotPkgs.pkg-config

        # For confirming the GPU is actually reachable before trusting any
        # offscreen-render timing -- see NIXOS_DEV_ENVIRONMENT.md's first-day
        # checklist. If `glxinfo | grep renderer` says llvmpipe here, the
        # NixOS SYSTEM config's nvidia driver setup needs fixing first (this
        # devShell can't supply a kernel driver, only point at one via
        # /run/opengl-driver/lib -- same mechanism the CUDA shell above uses).
        godotPkgs.glxinfo
        godotPkgs.vulkan-tools
      ];

      JAVA_HOME = "${godotPkgs.jdk21}";
      ANDROID_SDK_ROOT = "${androidSdk}/share/android-sdk";
      ANDROID_HOME = "${androidSdk}/share/android-sdk";

      CC_aarch64_linux_android = "${ndkBin}/aarch64-linux-android21-clang";
      CXX_aarch64_linux_android = "${ndkBin}/aarch64-linux-android21-clang++";
      AR_aarch64_linux_android = "${ndkBin}/llvm-ar";
      CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = "${ndkBin}/aarch64-linux-android21-clang";

      shellHook = ''
        export LD_LIBRARY_PATH="/run/opengl-driver/lib:$LD_LIBRARY_PATH"
        echo "street-smarts Godot dev shell (untested flake -- see NIXOS_DEV_ENVIRONMENT.md)"
        echo "  rustc:  $(rustc --version)"
        echo "  NDK bin: ${ndkBin}"
        echo ""
        echo "Real GPU check (must NOT say llvmpipe):"
        glxinfo 2>/dev/null | grep "OpenGL renderer" || echo "  glxinfo found nothing -- is a display available?"
      '';
    };
  };
}
