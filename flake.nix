{
  description = "street-smarts dev shell — CUDA 12.6 for ferrotorch on Maxwell (sm_50)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";

  outputs = { self, nixpkgs }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
      config.allowUnfree = true;
      config.cudaSupport = true;
    };
    cudaPackages = pkgs.cudaPackages_12_6;
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
        export LD_LIBRARY_PATH="/run/opengl-driver/lib:${cudaPackages.cuda_cudart}/lib:${cudaPackages.libcublas.lib}/lib:${cudaPackages.libcusolver.lib}/lib:${cudaPackages.libcufft.lib}/lib:${cudaPackages.libcusparse.lib}/lib:$LD_LIBRARY_PATH"
        echo "street-smarts dev shell — CUDA 12.6, Maxwell (sm_50)"
        echo "  CUDARC_CUDA_VERSION=$CUDARC_CUDA_VERSION"
        echo "  GPU: $(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null || echo 'nvidia-smi not found')"
      '';
    };
  };
}
