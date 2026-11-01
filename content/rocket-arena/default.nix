# Rocket Arena 3 for a content image, in two directories:
#
# - arena/: RA3 itself. Its game code on the server is only a 32-bit x86
#   library, so servers run it with game.native.
# - cpma/: RA3's maps for CPMA, whose Clan Arena is Rocket Arena's game mode.
#
# RA3 is freely distributable along with its readme, unchanged, which is
# included. Only the maps that are complete, or close to it, without the full
# game are included, with stand-ins from OpenArena for the full game's textures
# they use (see standins/).
{
  lib,
  stdenvNoCC,
  fetchurl,
  unzip,
  zip,
}:

let
  ra3 = fetchurl {
    url = "https://www.mirrorservice.org/sites/quakeunity.com/modifications/rocketarena3/ra3176.zip";
    hash = "sha256-yNoO3FWWHjKNzDXo17MXtjmwcSmJfNOQgBUUdct9o0o=";
  };
  openarena = fetchurl {
    url = "https://download.tuxfamily.org/openarena/rel/088/openarena-0.8.8.zip";
    hash = "sha256-Wo+vf1tR81GwoWGMBra5il8aZ1jx05gY3iyH3yoLrEo=";
  };
  maps = [
    "ra3map9"
    "ra3map12"
    "ra3map16"
    "ra3map17"
    "ra3map18"
    "ra3map20"
  ];
in
stdenvNoCC.mkDerivation {
  pname = "rocket-arena-content";
  version = "1.76";
  dontUnpack = true;
  nativeBuildInputs = [
    unzip
    zip
  ];
  installPhase = ''
    mkdir -p $out/arena
    unzip -q -j ${ra3} ${lib.concatMapStringsSep " " (m: "arena/${m}.pk3") maps} \
      arena/browser.pk3 arena/gfx.pk3 arena/icons.pk3 arena/menu.pk3 arena/models.pk3 \
      arena/scripts.pk3 arena/sounds.pk3 arena/textures.pk3 arena/vm.pk3 \
      arena/qagamei386.so arena/arena.cfg arena/readme.txt -d $out/arena
    # ioquake3 names 32-bit x86 libraries x86 rather than id's i386.
    mv $out/arena/qagamei386.so $out/arena/qagamex86.so
    sh ${../openarena-standins.sh} ${openarena} ${./standins/images.txt} \
      ${./standins/standins.shader} $out/arena/zz-openarena-standins.pk3

    mkdir -p $out/cpma
    # The maps, with RA3's textures and shaders they use, and its readme.
    unzip -q -j ${ra3} ${lib.concatMapStringsSep " " (m: "arena/${m}.pk3") maps} \
      arena/textures.pk3 arena/scripts.pk3 arena/readme.txt -d $out/cpma
    mv $out/cpma/textures.pk3 $out/cpma/ra3-textures.pk3
    mv $out/cpma/scripts.pk3 $out/cpma/ra3-scripts.pk3
    mv $out/cpma/readme.txt $out/cpma/ra3-readme.txt
    sh ${../openarena-standins.sh} ${openarena} ${./standins/images.txt} \
      ${./standins/standins.shader} $out/cpma/zz-ra3-openarena-standins.pk3
  ''
  # Which files the browser client reads on each map, from task content:profile,
  # for the rocket-arena preset and for RA3 itself.
  + lib.optionalString (builtins.pathExists ./profile.json) ''
    cp ${./profile.json} $out/cpma/quake-kube-profile-rocket-arena.json
  ''
  + lib.optionalString (builtins.pathExists ./profile-ra3.json) ''
    cp ${./profile-ra3.json} $out/arena/quake-kube-profile.json
  '';
  dontFixup = true;
}
