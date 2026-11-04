# The browser preview on the docs site (see hack/play-site). Its paks and web
# client are links to their packages, so this stays small. OpenArena's are on
# GitHub Pages, and hack/play-upload.sh puts the mods', which don't fit, on R2.
{
  lib,
  buildGoApplication,
  go_1_27,
  runCommand,
  webClient,
  openarena,
  cpma,
  urbanTerror,
}:

let
  tool = buildGoApplication {
    pname = "play-site";
    version = "0.1";
    src = lib.fileset.toSource {
      root = ../.;
      fileset = lib.fileset.unions [
        ../go.mod
        ../go.sum
        ../internal
        ../hack/play-site
        # The test builds the real pages.
        ../controllers/static
      ];
    };
    modules = ../gomod2nix.toml;
    subPackages = [ "./hack/play-site" ];
    go = go_1_27;
    CGO_ENABLED = 0;
  };
in
runCommand "quake-kube-play-site" { } ''
  ${tool}/bin/play-site \
    --static ${../controllers/static} \
    --client ${webClient} \
    --client-version "$(basename ${webClient})" \
    --content ${openarena} \
    --content ${cpma} \
    --content ${urbanTerror} \
    --remote cpma=https://content.chrisrx.dev \
    --remote q3ut4=https://content.chrisrx.dev \
    --presets ${../hack/k3d/presets.yaml} \
    --out $out
''
