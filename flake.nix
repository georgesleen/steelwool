{
  description = "Opinionated formatter for Steel";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      forAllSystems = nixpkgs.lib.genAttrs nixpkgs.lib.systems.flakeExposed;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        rec {
          steelwool = pkgs.rustPlatform.buildRustPackage {
            pname = "steelwool";
            version = "0.2.0";
            src = nixpkgs.lib.fileset.toSource {
              root = ./.;
              fileset = nixpkgs.lib.fileset.gitTracked ./.;
            };
            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes."steel-parser-0.8.3" = "sha256-AMDFQcukZUFV7U5mQaHBNsGpcGoIKE4Ic+IbXBNGcFI=";
            };
            meta = {
              description = "Opinionated formatter for Steel";
              homepage = "https://github.com/georgesleen/steelwool";
              license = nixpkgs.lib.licenses.lgpl3Plus;
              mainProgram = "steelwool";
            };
          };
          default = steelwool;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              git
              gnumake
              nixfmt
              rustc
              cargo
              clippy
              rustfmt
              rust-analyzer
              # steel: the interpreter the formatted sources run under.
              steel
            ];
          };
        }
      );
    };
}
