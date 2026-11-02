# Community maps for Quake 3 that work with the demo's content, for a content
# image: its baseq3/ directory. Their readmes allow distributing them for free,
# unaltered, so the pk3s are shipped as they are, with the rest of each zip, its
# readme, in readmes/. Stand-ins from OpenArena cover the full game's textures
# some of them use (see standins/). See the docs' Content Removal section for
# asking for a map to be taken out.
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
  # Each map's zip, by its main pk3. All of a zip's pk3s are taken, e.g. a patch
  # shipped with the map.
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
    # Evolution, suboptimal, The Hot Place and Tortured by thefury.
    "pukka3tourney2.pk3" =
      map "m-r" "pukka3tourney2" "sha256-Wzpl8iTiuhNnIaq4wNYHv6O+3Rs2CukUSkj1vNgSO7w=";
    "pukka3tourney5.pk3" =
      map "m-r" "pukka3tourney5" "sha256-+h56k5qHT5Z7u5CuKOoyvjKvngtlRgeVe0eTMm7Bqhc=";
    "pukka3dm1.pk3" = map "m-r" "pukka3dm1" "sha256-+CKDu0gPZU12BQJtQIq8HB0AIUgjWdHqzi/sIifJp74=";
    "pukka3tourney7.pk3" =
      map "m-r" "pukka3tourney7" "sha256-L/RbpHKgZGk3kR+ccvrBYMeDSLjzodxRhktRz1Mt21I=";
    # ALIEN, Saxa Liquefacta, Magnetic Centrifuge, Core Gravity Drive, Deep Purple
    # and Running Man by AKUTA.
    "akutadm1.pk3" = map "a-f" "akutadm1" "sha256-zDLCJQn/N2s6r8ZduU1rp2wtnOBpRscH0+OIJns11gw=";
    "akutadm2.pk3" = map "a-f" "akutadm2" "sha256-KlSPmZY3gCGiW0JIt8324gWIn29bLb2fAfGSTsDRBFg=";
    "akutadm3.pk3" = map "a-f" "akutadm3" "sha256-qL3vSWNqbH22mz1UZ6RagDRzeDboVIMSyJKU9nUnyQ0=";
    "akutatourney5.pk3" =
      map "a-f" "akutatourney5" "sha256-tbIw84qRhzmawYGryeHnvjPTueceb1T9r1UDWWzBOLA=";
    "akutatourney6.pk3" =
      map "a-f" "akutatourney6" "sha256-L5KnchxE9lsW4R20rHmHdFaInkJN7nQ2GQJ833YNCN4=";
    "akutatourney8.pk3" =
      map "a-f" "akutatourney8" "sha256-3u9wJCKTbgTCdNoVVecuXF2wuT8QyYwwgW4WyT4YLY8=";
    # Terminatria by bst.
    "bst3dm1.pk3" = map "a-f" "bst3dm1" "sha256-k/DBQU1NUfCvty9i6LkBU90z87OLvGOGQyKdzfGniI4=";
    # Rainy Day by cityy. Its pk3 also has pro-ct3tourney2, a variant.
    "ct3tourney2.pk3" = map "a-f" "ct3tourney2" "sha256-gOTgvnbSuSCZN4onYMiGJt7rRlX06qKeMNILTUOIOOg=";
    # TNK gas station by Anton. Its Team Arena nailgun is left out in Quake 3.
    "gas-st.pk3" = map "g-l" "gas-st_pka" "sha256-SpiTB4LSnehoLoldTYP4wTHNhA6Q6CmrA/eS7+VN3XE=";
    # Ballistophobia by Jester.
    "jstrq3dm2.pk3" = map "g-l" "jstrq3dm2" "sha256-uWNxFhnuCamfKUkgjF0ouQwLbmB5k4wmH3g4HR3aWkM=";
    # Inner Sanctum by kat.
    "map-katdm3.pk3" = map "m-r" "map-katdm3" "sha256-S/H7pLbayUfm01mS6p231J10/tn+SYSCyRUzaC6Bixo=";
    # Winter Games by Comic Relief.
    "map-wintergames.pk3" =
      map "s-z" "wintergames" "sha256-Fd8wB4gc3XJ6kEr5dbXUjzf+tPxeReTnPmHnYA1CUXA=";
    # Hypersonic Tourney by zeal0r.
    "zl3tourney1.pk3" = map "s-z" "zl3tourney1" "sha256-TEtq9vS63drdHl21khLrGwH+G5vodGPgiNAwQkM6K5U=";
    # Hot and Moist by pjw. Most of its gothic textures stand in under other
    # names (see standins/renamed.txt).
    "pjw3tourney1.pk3" =
      map "m-r" "pjw3tourney1" "sha256-XxziqzGIrI/jmK1urrlNN/BzsXkbqkDiaKOyJ/inrxw=";
    # Hydra and Goldleaf by Pat Howard. Their readmes ask for the author's consent
    # to redistribute them, but he can't be reached, so they're shipped as
    # orphaned works, to be removed as soon as he asks (see the docs' Content
    # Removal section).
    "hydra.pk3" = map "g-l" "hydra" "sha256-yxG7EPiZODL+glGn6nHCOJPw6jR/nmGxSM4x+mwYh/g=";
    "goldleaf.pk3" = map "g-l" "goldleaf" "sha256-LkVtugRdLYsNgZ1WyGkpRkGu7hXqE8scOlRGRq7v4Hs=";
    # Endurance by flipout.
    "q3gwdm2.pk3" = map "m-r" "q3gwdm2" "sha256-s6+CFo4GEe9OJtVin/aFJ3aTvvm3VMK8CTh0U8ze3Qc=";
    # Windsong Keep and Corrosion by Phantazm11.
    "phantq3dm4.pk3" = map "m-r" "phantq3dm4" "sha256-Jkt2gAyQ8u0NzImbH1CgpztAb84Dt3ne8+aty5amTnc=";
    "phantq3dm3_rev.pk3" = map "m-r" "phantq3dm3_rev" "sha256-emoOquc7Fq2tijw4qjaI8/iWFsB2jlQKJbnaq0KBYUE=";
    # Pyramid of the Magician by Sock.
    "pom_bots.pk3" = map "m-r" "pom_bots" "sha256-j8bXvS70jkpvN7o7b6zx1XgBaG+Xda5mEg3zUYkDCLc=";
    # Dynasty by EmeraldTiger. Its Team Arena items are left out in Quake 3.
    "pasdm1.pk3" = map "m-r" "pasdm1" "sha256-vI0zOhOlCgjf5beY2A5Z2VWXROzWtc46DOfbyb46Zrg=";
    # Tatatu by GrindSpire.
    "gs3dm5.pk3" = map "g-l" "gs3dm5" "sha256-OU0TGjyOT0/5fZP5Ocyq2ltIYGCzK76Vc+LHBq1HD8c=";
    # The Forlorn Hope by JustOneFiX.
    "jof3dm2.pk3" = map "g-l" "jof3dm2" "sha256-YOyog5uKDyeyoltXoft5IWSxDn1uaThTWHJMTSSFMME=";
    # Grindill 2.0 by Quaker-X.
    "qxdm4_v2.pk3" = map "m-r" "qxdm4_v2" "sha256-FshCN14SExrHXEG1SP8L2bmOyaNPiWVBn8NXiRBmeBE=";
    # Left Behind by cityy.
    "ct3dm5.pk3" = map "a-f" "ct3dm5" "sha256-VsqtiA5bao1HffznaAYWVBEJoTdzqpgPcqVMj57mk3A=";
    # Revenga! by Strahlemann.
    "revenga.pk3" = map "m-r" "revenga" "sha256-hzLEr02l3Xd0A22z4j4uFssGs0CzbEDA0et3J2L0OgI=";
    # PowerStroke and Military Complex by AKUTA.
    "akutatourney3.pk3" = map "a-f" "akutatourney3" "sha256-VGh/h7VqxKk5RxSMcIewJ3RPHv5Y8DwIp08J2t28z60=";
    "akutatourney4.pk3" = map "a-f" "akutatourney4" "sha256-RhkgsGJVDGRqU9llP2IRUnqLWPWmsXlYCEws55CIK5U=";
    # Deep Freeze by Shadowdane, with lvlworld's patch for its sky.
    "shad3dm2.pk3" = map "s-z" "shad3dm2" "sha256-N1//iN0WCH+dQXA88F4/bhRNqKOcsI1TCYMgYW3IA3o=";
    # Blood Run Tourney by ztn. Most of its gothic textures stand in under other
    # names (see standins/renamed.txt).
    "ztn3tourney1.pk3" = map "s-z" "ztn3tourney1" "sha256-yKLRhqKG6dlfhwZuQ4pOUkfqpYEIecp7L6stKkSlU2Q=";
    # Capture the flag: Derwyll's Castle 2 by Noruen, and Substation 11 by
    # Phantazm11.
    "nor3ctf1.pk3" = map "m-r" "nor3ctf1" "sha256-4FhOGbhZsYPG4ZbCPra+K0Gxv/etku5yw+ItrmLFweE=";
    "phantq3ctf1_rev.pk3" = map "m-r" "phantq3ctf1_rev" "sha256-hVGsuRaMldxGf9OeTYbIB6EF/q1ho4mXMXrWbFwtvx0=";
    # Geotechnic by Phantazm11.
    "phantq3dm6_mc.pk3" = map "m-r" "phantq3dm6_mc" "sha256-QTT2RTemJetcYegrbzUAJ5weECaDNFSMsDAgU5cAPF0=";
    # Achromatic by flipout.
    "q3gwdm1.pk3" = map "m-r" "q3gwdm1" "sha256-HjRyk9+tPTiBBQZMYUcj2wbB2gbop7Hx8Od1kiB7U74=";
    # The Edge of Forever by Sock, whose map is moteof_final. It has no bot file.
    "moteof_final2.pk3" = map "m-r" "moteof_final2" "sha256-XsdWRtd5BvjrgqhrtNAQhgJuGnKX8Asa+Ob6/a1/f/w=";
    # Trespass, and Ironwood for capture the flag, by Pat Howard, shipped as
    # orphaned works like Hydra and Goldleaf.
    "trespass.pk3" = map "s-z" "trespass" "sha256-iHGNdPbCdidLzvoWNMKerICoVy5WZ8Ritu0Oz7aofLo=";
    "ironwood.pk3" = map "g-l" "ironwood" "sha256-vyKNu5oazs/PRmvHG41UbiXAEa5gW7crpJbg76kUCIU=";
  };
  openarena = fetchurl {
    url = "https://download.tuxfamily.org/openarena/rel/088/openarena-0.8.8.zip";
    hash = "sha256-Wo+vf1tR81GwoWGMBra5il8aZ1jx05gY3iyH3yoLrEo=";
  };
in
stdenvNoCC.mkDerivation {
  pname = "quake3-maps-content";
  version = "5";
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
      unzip -q -j ${zip} '*.pk3' -d $out/baseq3
      mkdir -p $out/baseq3/readmes/${lib.removeSuffix ".pk3" pk3}
      unzip -q -j ${zip} -x '*.pk3' -d $out/baseq3/readmes/${lib.removeSuffix ".pk3" pk3}
    '') maps
  )
  + ''
    sh ${../openarena-standins.sh} ${openarena} ${./standins/images.txt} ${./standins/standins.shader} \
      $out/baseq3/zz-quake3-maps-openarena-standins.pk3
    # Stand-ins under other names, for textures OpenArena has no same-named
    # replacement for.
    mkdir -p renamed openarena
    grep -v '^#' ${./standins/renamed.txt} | while read -r pak src dest; do
      [ -f openarena/$pak ] || unzip -q -j ${openarena} openarena-0.8.8/baseoa/$pak -d openarena
      mkdir -p renamed/$(dirname $dest)
      unzip -q -p openarena/$pak $src > renamed/$dest
    done
    (cd renamed && zip -q -r -X $out/baseq3/zz-quake3-maps-openarena-standins.pk3 .)
  ''
  # Which files the browser client reads on each map, from task content:profile.
  + lib.optionalString (builtins.pathExists ./profile.json) ''
    cp ${./profile.json} $out/baseq3/quake-kube-profile.json
  '';
  dontFixup = true;
}
