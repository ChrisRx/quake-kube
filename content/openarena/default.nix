# OpenArena 0.8.8 and the OpenArena Community Mappack Vol. 1 for a content
# image: its baseoa/ directory. Both are GPLv2.
{
  lib,
  stdenvNoCC,
  fetchurl,
  fetchFromGitHub,
  unzip,
  zip,
}:

let
  openarena = fetchurl {
    url = "https://download.tuxfamily.org/openarena/rel/088/openarena-0.8.8.zip";
    hash = "sha256-Wo+vf1tR81GwoWGMBra5il8aZ1jx05gY3iyH3yoLrEo=";
  };
  mappack = fetchFromGitHub {
    owner = "NeonKnightOA";
    repo = "oacmpvol1";
    rev = "25c9fa41acaf6b91481fab5183e901467c594e0c";
    hash = "sha256-WFiodlkfr79CAFa3KuC+pefy1nhnu/7aI5Curw72xc4=";
  };
in
stdenvNoCC.mkDerivation {
  pname = "openarena-content";
  version = "0.8.8";
  dontUnpack = true;
  nativeBuildInputs = [
    unzip
    zip
  ];
  installPhase = ''
    mkdir -p $out/baseoa
    # The optional mature player skins are left out, which clients must
    # match on pure servers.
    unzip -q -j ${openarena} 'openarena-0.8.8/baseoa/*' -x '*/pak2-players-mature.pk3' -d $out/baseoa
    unzip -q -j ${openarena} openarena-0.8.8/COPYING -d $out/baseoa

    # The map pack as its own build script packs it.
    (cd ${mappack}/baseoa && zip -q -X -9 -r $out/baseoa/z_oacmp1.pk3 \
      levelshots maps/*.aas maps/*.bsp models music scripts/*.arena scripts/*.shader \
      sound textures)
    # Its license, the maps' readmes and permissions for borrowed textures.
    (cd ${mappack} && zip -q -X -9 -r $out/baseoa/z_oacmp1.pk3 docs)
  ''
  # Which files the browser client reads on each map, from task content:profile.
  + lib.optionalString (builtins.pathExists ./profile.json) ''
    cp ${./profile.json} $out/baseoa/quake-kube-profile.json
  '';
  dontFixup = true;
}
