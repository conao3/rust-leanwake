{
  description = "Leanwake: command elaboration profiles for Lake projects";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
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
        { pkgs, ... }:
        let
          adapter = pkgs.stdenv.mkDerivation {
            pname = "leanwake-adapter";
            version = "0.1.0";
            src = ./native;
            nativeBuildInputs = [
              pkgs.lean4
              pkgs.makeWrapper
            ];
            buildPhase = "lake build";
            installPhase = ''
              install -Dm755 .lake/build/bin/leanwake-adapter $out/bin/leanwake-adapter
              install -Dm755 .lake/build/bin/leanwake-project $out/bin/leanwake-project
              wrapProgram $out/bin/leanwake-project --set LEAN_SYSROOT ${pkgs.lean4}
              wrapProgram $out/bin/leanwake-adapter --set LEAN_SYSROOT ${pkgs.lean4}
            '';
          };
        in
        {
          packages.adapter = adapter;
          packages.default = pkgs.rustPlatform.buildRustPackage {
            pname = "leanwake";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [ pkgs.makeWrapper ];
            nativeCheckInputs = [ pkgs.lean4 ];
            LEANWAKE_ADAPTER = "${adapter}/bin/leanwake-adapter";
            checkPhase = "cargo test -j 4 -- --test-threads=1";
            LEAN_SYSROOT = "${pkgs.lean4}";
            postInstall = ''
              wrapProgram $out/bin/leanwake --set LEANWAKE_ADAPTER ${adapter}/bin/leanwake-adapter --set LEAN_SYSROOT ${pkgs.lean4} --prefix PATH : ${
                pkgs.lib.makeBinPath [ pkgs.lean4 ]
              }
            '';
            meta = {
              mainProgram = "leanwake";
              license = pkgs.lib.licenses.asl20;
            };
          };
          devShells.default = pkgs.mkShell {
            packages = [
              pkgs.cargo
              pkgs.rustc
              pkgs.rustfmt
              pkgs.clippy
              pkgs.rust-analyzer
              pkgs.lean4
              pkgs.gnumake
              pkgs.python3
              pkgs.ruff
            ];
            LEAN_SYSROOT = "${pkgs.lean4}";
            shellHook = ''export LEANWAKE_ADAPTER="$PWD/native/.lake/build/bin/leanwake-adapter"'';
          };
          treefmt.programs.nixfmt.enable = true;
          treefmt.programs.rustfmt.enable = true;
          treefmt.programs.ruff-check.enable = true;
          treefmt.programs.ruff-format.enable = true;
        };
    };
}
