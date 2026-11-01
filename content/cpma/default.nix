# Challenge ProMode Arena 1.53 and its map pack for a content image: its cpma/
# directory, with a stand-in for its default player model from OpenArena (see
# openarena-mynx.sh).
{
  lib,
  stdenvNoCC,
  fetchurl,
  unzip,
  zip,
}:

let
  cpma = fetchurl {
    url = "https://cdn.playmorepromode.com/files/cpma/cpma-1.53-nomaps.zip";
    hash = "sha256-7f/6DBoDdbpGpbQiV6Fo+xUIZxIkVzNSarLZzN2CHKA=";
  };
  maps = fetchurl {
    url = "https://cdn.playmorepromode.com/files/cpma-mappack-full.zip";
    hash = "sha256-Xbkz/JLEHy4JQatlclWG1NDDD+hHJ0J7trJl5NlBoiY=";
  };
  openarena = fetchurl {
    url = "https://download.tuxfamily.org/openarena/rel/088/openarena-0.8.8.zip";
    hash = "sha256-Wo+vf1tR81GwoWGMBra5il8aZ1jx05gY3iyH3yoLrEo=";
  };
in
stdenvNoCC.mkDerivation {
  pname = "cpma-content";
  version = "1.53";
  dontUnpack = true;
  nativeBuildInputs = [
    unzip
    zip
  ];
  installPhase = ''
    mkdir -p $out/cpma
    unzip -q ${cpma} -d $out
    # The map pack's pk3s are at the top of its archive.
    unzip -q ${maps} -d $out/cpma
    unzip -q -j ${openarena} \
      openarena-0.8.8/baseoa/pak2-players.pk3 openarena-0.8.8/COPYING -d openarena
    sh ${./openarena-mynx.sh} openarena/pak2-players.pk3 $out/cpma/zz-openarena-mynx.pk3
    cp openarena/COPYING $out/cpma/zz-openarena-mynx-COPYING.txt
  ''
  # Which files the browser client reads on each map, from task content:profile.
  + lib.optionalString (builtins.pathExists ./profile.json) ''
    cp ${./profile.json} $out/cpma/quake-kube-profile.json
  '';
  dontFixup = true;
}
