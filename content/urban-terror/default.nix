# Urban Terror 4.3.4's game files for a content image: its q3ut4/ directory.
{
  lib,
  stdenvNoCC,
  fetchurl,
  unzip,
}:

stdenvNoCC.mkDerivation {
  pname = "urban-terror-content";
  version = "4.3.4";
  src = fetchurl {
    url = "https://cdn.urbanterror.info/urt/43/releases/zips/UrbanTerror434_full.zip";
    hash = "sha256-cWZpzGxSVmO3kYUuUf+clNlvOCvjknnatghN3ay1pOc=";
  };
  nativeBuildInputs = [ unzip ];
  unpackPhase = ''
    unzip -q $src 'UrbanTerror43/q3ut4/*'
  '';
  installPhase = ''
    mkdir -p $out
    mv UrbanTerror43/q3ut4 $out/
  ''
  # Which files the browser client reads on each map, from task content:profile.
  + lib.optionalString (builtins.pathExists ./profile.json) ''
    cp ${./profile.json} $out/q3ut4/quake-kube-profile.json
  '';
  dontFixup = true;
}
