# Urban Terror 4.3.4's game files for a content image: its q3ut4/ directory.
{ stdenvNoCC, fetchurl, unzip }:

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
  '';
  dontFixup = true;
}
