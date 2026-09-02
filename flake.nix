{
  description = "Grok Build — prebuilt GitHub Release binaries";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.nix-rust-prebuilt.url = "github:yangtau/nix-rust-prebuilt";

  outputs =
    { self, nixpkgs, nix-rust-prebuilt }:
    let
      inherit (nixpkgs) lib;
      systems = [ "aarch64-darwin" "x86_64-linux" ];
      meta = {
        description = "Grok Build TUI (prebuilt from yangtau/grok-build releases)";
        homepage = "https://github.com/yangtau/grok-build";
        license = lib.licenses.asl20;
        sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
      };

      packages = nix-rust-prebuilt.lib.mkPackages {
        inherit self nixpkgs meta systems;
        pname = "grok";
        owner = "yangtau";
        repo = "grok-build";
        hashes = ./.nix/prebuilt-hashes.json;
        # Cannot compile in Nix (private async-openai). Download whenever a hash exists.
        requireRev = false;
        fromSource = null;
        overridePrebuilt =
          pkgs: drv:
          drv.overrideAttrs (old: {
            nativeBuildInputs = (old.nativeBuildInputs or [ ]) ++ [ pkgs.makeWrapper ];
            buildInputs = (old.buildInputs or [ ]) ++ lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.zlib ];
            installPhase = ''
              runHook preInstall
              mkdir -p $out/libexec $out/bin
              tar -xzf $src -C $out/libexec
              test -x $out/libexec/grok
              makeWrapper $out/libexec/grok $out/bin/grok \
                --argv0 grok \
                --add-flags --no-auto-update
              runHook postInstall
            '';
          });
      };
    in
    {
      inherit packages;

      apps = lib.genAttrs systems (system: {
        default = {
          type = "app";
          program = "${packages.${system}.default}/bin/grok";
        };
      });

      overlays.default = final: prev: {
        grok-build = packages.${final.stdenv.hostPlatform.system}.default;
      };
    };
}
