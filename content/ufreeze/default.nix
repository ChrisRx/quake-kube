# Ultra Freeze Tag 1.21 (core 57) for a content image: its ufreeze/ directory,
# with stand-ins from OpenArena for the full game's files its game code uses,
# like the CTF flags and the grappling hook (see standins/). The optional skin
# paks are left out: pmskins.pk3 is CPMA's skins.
{
  stdenvNoCC,
  fetchurl,
  unzip,
  zip,
}:

let
  ufreeze = fetchurl {
    url = "https://www.mirrorservice.org/sites/quakeunity.com/modifications/ultrafreezetag/Ufreeze121core57.zip";
    hash = "sha256-k83Zq6+HNuk8l0hu4gNhOCIFXJFY/eK7uTv+OnBVuJk=";
  };
  openarena = fetchurl {
    url = "https://download.tuxfamily.org/openarena/rel/088/openarena-0.8.8.zip";
    hash = "sha256-Wo+vf1tR81GwoWGMBra5il8aZ1jx05gY3iyH3yoLrEo=";
  };
in
stdenvNoCC.mkDerivation {
  pname = "ufreeze-content";
  version = "1.21";
  dontUnpack = true;
  nativeBuildInputs = [
    unzip
    zip
  ];
  installPhase = ''
    mkdir -p $out/ufreeze
    unzip -q -j ${ufreeze} \
      ufreeze/ufreeze-57core.pk3 ufreeze/ufreeze-55gfx.pk3 ufreeze/ufreeze-voc.pk3 \
      -d $out/ufreeze
    unzip -q -j ${ufreeze} Readme.txt -d $out/ufreeze
    mv $out/ufreeze/Readme.txt $out/ufreeze/ufreeze-readme.txt
    sh ${../openarena-standins.sh} ${openarena} ${./standins/images.txt} - \
      $out/ufreeze/zz-openarena-standins.pk3
  '';
  dontFixup = true;
}
