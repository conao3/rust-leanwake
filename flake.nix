{
  description = "Leanwake: find the slowest proofs in Lake projects";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    leanfran.url = "github:conao3/lean-leanfran/420a7dce66dda3cbfd03b884d453cb8de63a0fc6";
    leanfran.inputs.nixpkgs.follows = "nixpkgs";
    flake-parts.url = "github:hercules-ci/flake-parts";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.treefmt-nix.flakeModule ];
      systems = [
        "x86_64-linux"
        "aarch64-darwin"
      ];
      perSystem =
        {
          config,
          pkgs,
          system,
          ...
        }:
        let
          lean =
            assert pkgs.lib.assertMsg (
              pkgs.lean4.version == "4.35.0"
            ) "Leanwake requires nixpkgs Lean 4.35.0; update the lock file deliberately";
            pkgs.lean4;
          adapter = pkgs.stdenv.mkDerivation {
            pname = "leanwake-adapter";
            version = "0.2.0";
            src = ./native;
            nativeBuildInputs = [
              lean
              pkgs.makeWrapper
            ];
            buildPhase = "lake build";
            installPhase = ''
              install -Dm755 .lake/build/bin/leanwake-adapter $out/bin/leanwake-adapter
              install -Dm755 .lake/build/bin/leanwake-project $out/bin/leanwake-project
              wrapProgram $out/bin/leanwake-project --set LEAN_SYSROOT ${lean}
              wrapProgram $out/bin/leanwake-adapter --set LEAN_SYSROOT ${lean}
            '';
          };
        in
        {
          packages.adapter = adapter;
          packages.default = pkgs.rustPlatform.buildRustPackage {
            pname = "leanwake";
            version = "0.2.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [ pkgs.makeWrapper ];
            nativeCheckInputs = [ lean ];
            LEANFRAN_OFFLINE = "1";
            LEANWAKE_ADAPTER = "${adapter}/bin/leanwake-adapter";
            checkPhase = "cargo test -j 4 -- --test-threads=1";
            LEAN_SYSROOT = "${lean}";
            postInstall = ''
              wrapProgram $out/bin/leanwake --set LEANWAKE_ADAPTER ${adapter}/bin/leanwake-adapter --set LEAN_SYSROOT ${lean} --prefix PATH : ${pkgs.lib.makeBinPath [ lean ]} --suffix PATH : ${
                pkgs.lib.makeBinPath [
                  pkgs.stdenv.cc
                  pkgs.git
                ]
              }
            '';
            doInstallCheck = true;
            nativeInstallCheckInputs = [
              pkgs.python3
              pkgs.git
            ];
            installCheckPhase = ''
              python3 tools/check-package.py "$out/bin/leanwake"
            '';
            meta = {
              description = "Find the slowest proofs in Lean 4.35.0 Lake projects and compare elaboration profiles";
              mainProgram = "leanwake";
              license = pkgs.lib.licenses.asl20;
            };
          };
          devShells.default = pkgs.mkShell {
            packages = [
              inputs.leanfran.packages.${system}.leanfran
              pkgs.cargo
              pkgs.rustc
              pkgs.rustfmt
              pkgs.clippy
              pkgs.rust-analyzer
              lean
              pkgs.gnumake
              pkgs.python3
              pkgs.ruff
            ];
            LEAN_SYSROOT = "${lean}";
            LEANFRAN_CLI = "${inputs.leanfran.packages.${system}.leanfran}/bin/leanfran";
            shellHook = ''export LEANWAKE_ADAPTER="$PWD/native/.lake/build/bin/leanwake-adapter"'';
          };
          treefmt.flakeFormatter = false;
          formatter = pkgs.writeShellApplication {
            name = "format-tracked";
            runtimeInputs = [
              pkgs.git
              pkgs.findutils
            ];
            text = ''
              cd "$(git rev-parse --show-toplevel)"
              git ls-files --cached -z | xargs -0 -r ${config.treefmt.build.wrapper}/bin/treefmt "$@"
            '';
          };
          treefmt.programs.nixfmt.enable = true;
          treefmt.programs.rustfmt.enable = true;
          treefmt.programs.ruff-check.enable = true;
          treefmt.programs.ruff-format.enable = true;
        };
    };
}
