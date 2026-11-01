# Community maps for Quake 3 that work with the demo's content, for a content
# image: its baseq3/ directory. Their readmes, which are inside their pk3s,
# allow distributing them for free, unaltered, so the pk3s are shipped as they
# are. Stand-ins from OpenArena cover the full game's textures some of them use
# (see standins/).
#
# The zips come from lvlworld's FTP mirror, whose paths don't need a token.
{
  lib,
  stdenvNoCC,
  fetchurl,
  unzip,
  zip,
}:

let
  map =
    dir: name: hash:
    fetchurl {
      url = "ftp://lvlmirror.mhgaming.com/${dir}/${name}.zip";
      inherit hash;
    };
  # The pk3 to take from each map's zip.
  maps = {
    # Q3Deck by kinetix.
    "q3deck.pk3" = map "m-r" "q3deck_final" "sha256-xMjcUAg77nSHItD0gSGZBdTHNDgMnv5e7GODrGWczXw=";
    # ChiropteraTA by Alcatraz, nunuk and Sock, made for Team Arena. Its Team
    # Arena weapons and ammo are left out in Quake 3.
    "chiropteraTA.pk3" =
      map "a-f" "chiropterata" "sha256-sFEvPfBIaQyv+GbzcBgEFIgBRZ+7W0Knv1GWB9QT26g=";
    # Desert Temple by LordSquart.
    "lsdm1.pk3" = map "g-l" "lsdm1" "sha256-MCZe+hKbdO8Z8x0642TIjor9Cw4Yky6jOlNZ7WtQVmY=";
    # Estatica by Cardigan.
    "map-estatica-cardigan.pk3" =
      map "m-r" "map-estatica-cardigan" "sha256-ILAQ0UQ4hxc/+5PWFr9hzaMOnf5/4p8R+29zNP0MJlQ=";
    # Trial by Error by R.P.G., under the GPL.
    "rpg3dm2.pk3" = map "m-r" "rpg3dm2" "sha256-7x74SbEW3a7ggHJ8TuzeBdT0NmntiA6wUwCnd5RZzhY=";
    # You'll Shoot Your Eye Out by Lunaran and KungFuSquirrel.
    "lun3dm5.pk3" = map "g-l" "lun3dm5" "sha256-vDPDH3Z5RmocVjnkK1uuQbcEsyIan9NyiIYoBKS5sew=";
  };
  openarena = fetchurl {
    url = "https://download.tuxfamily.org/openarena/rel/088/openarena-0.8.8.zip";
    hash = "sha256-Wo+vf1tR81GwoWGMBra5il8aZ1jx05gY3iyH3yoLrEo=";
  };
in
stdenvNoCC.mkDerivation {
  pname = "quake3-maps-content";
  version = "1";
  dontUnpack = true;
  nativeBuildInputs = [
    unzip
    zip
  ];
  installPhase = ''
    mkdir -p $out/baseq3
  ''
  + lib.concatStrings (
    lib.mapAttrsToList (pk3: zip: ''
      unzip -q -j ${zip} '*${pk3}' -d $out/baseq3
    '') maps
  )
  + ''
    sh ${../openarena-standins.sh} ${openarena} ${./standins/images.txt} - \
      $out/baseq3/zz-quake3-maps-openarena-standins.pk3
  ''
  # Which files the browser client reads on each map, from task content:profile.
  + lib.optionalString (builtins.pathExists ./profile.json) ''
    cp ${./profile.json} $out/baseq3/quake-kube-profile.json
  '';
  dontFixup = true;
}
