{
  description = "steelwool dev shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      forAllSystems = nixpkgs.lib.genAttrs nixpkgs.lib.systems.flakeExposed;
    in
    {
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
