# DeFRaG 1.91.31 for a content image: its defrag/ directory, with its five test
# maps, and stand-ins from OpenArena for the full game's files its game code
# and the test maps use (see standins/).
{
  stdenvNoCC,
  fetchurl,
  unzip,
  zip,
}:

let
  defrag = fetchurl {
    url = "https://q3defrag.org/files/defrag/defrag_1.91.31.zip";
    hash = "sha256-eifVaRjzFWOPUH32Ctq82CPQpRhIinGL8qc6deujb1M=";
  };
  openarena = fetchurl {
    url = "https://download.tuxfamily.org/openarena/rel/088/openarena-0.8.8.zip";
    hash = "sha256-Wo+vf1tR81GwoWGMBra5il8aZ1jx05gY3iyH3yoLrEo=";
  };
in
stdenvNoCC.mkDerivation {
  pname = "defrag-content";
  version = "1.91.31";
  dontUnpack = true;
  nativeBuildInputs = [
    unzip
    zip
  ];
  installPhase = ''
    mkdir -p $out/defrag
    # unzip's * matches across directories, so misc/ is left out explicitly.
    unzip -q -j ${defrag} 'defrag/*.pk3' defrag/readme.txt -x 'defrag/misc/*' -d $out/defrag
    # The test maps are left out of the game directory, for players to add.
    unzip -q -j ${defrag} defrag/misc/pk3s/zz-defrag_testmaps.pk3 -d $out/defrag
    sh ${../openarena-standins.sh} ${openarena} ${./standins/images.txt} - \
      $out/defrag/zz-openarena-standins.pk3
    # The test maps' sky is the full game's redclouds, which OpenArena has no
    # same-named stand-in for, so its red clouds stand in under that name.
    mkdir -p sky/textures/skies
    unzip -q -j ${openarena} openarena-0.8.8/baseoa/pak4-textures.pk3 -d openarena
    unzip -q -p openarena/pak4-textures.pk3 textures/skies/intelredclouds.jpg \
      > sky/textures/skies/redclouds.jpg
    (cd sky && zip -q -r -X $out/defrag/zz-openarena-standins.pk3 textures)
  '';
  dontFixup = true;
}
