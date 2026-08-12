# Prebuilt grok from this repo's GitHub Releases.
#
# hashes.json is written by .github/workflows/release.yml after a successful
# build. The flake never compiles the Rust workspace — that needs xAI's
# private async-openai fork and a huge closure.
{
  lib,
  stdenv,
  fetchurl,
  makeWrapper,
  autoPatchelfHook,
  zlib,
}:

let
  inherit (lib.importJSON ./hashes.json)
    tag
    version
    hashes
    ;

  system = stdenv.hostPlatform.system;

  triple =
    {
      aarch64-darwin = "aarch64-apple-darwin";
    }
    .${system} or (throw "grok: no prebuilt binary for ${system} (this fork ships aarch64-darwin only)");

  hash =
    hashes.${system} or (throw ''
      grok: no hash for ${system} in nix/hashes.json.
      Publish one first:  gh workflow run release.yml
    '');

in
assert tag != null || throw "grok: nix/hashes.json has no release tag yet (run the release workflow)";

stdenv.mkDerivation {
  pname = "grok";
  inherit version;

  src = fetchurl {
    url = "https://github.com/yangtau/grok-build/releases/download/${tag}/grok-${triple}.tar.gz";
    inherit hash;
  };

  nativeBuildInputs = [ makeWrapper ] ++ lib.optionals stdenv.hostPlatform.isLinux [ autoPatchelfHook ];

  buildInputs = lib.optionals stdenv.hostPlatform.isLinux [
    stdenv.cc.cc
    zlib
  ];

  # Already stripped in CI. Nix's strip can break rust/jemalloc binaries.
  dontStrip = true;

  sourceRoot = ".";

  installPhase = ''
    runHook preInstall
    install -Dm755 grok $out/libexec/grok
    makeWrapper $out/libexec/grok $out/bin/grok \
      --argv0 grok \
      --add-flags --no-auto-update
    runHook postInstall
  '';

  meta = {
    description = "Grok Build TUI (prebuilt from yangtau/grok-build releases)";
    homepage = "https://github.com/yangtau/grok-build";
    license = lib.licenses.asl20;
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
    mainProgram = "grok";
    platforms = builtins.attrNames hashes;
  };
}
