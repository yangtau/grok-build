{
  description = "Grok Build — prebuilt GitHub Release binaries";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          grok = pkgs.callPackage ./nix/package.nix { };
        in
        {
          inherit grok;
          default = grok;
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/grok";
        };
      });

      overlays.default = final: prev: {
        grok-build = final.callPackage ./nix/package.nix { };
      };
    };
}
