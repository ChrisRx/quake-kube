# The controller and gateway binary. Its dependencies are built separately, so
# changes to the controller don't rebuild them.
{ lib, craneLib }:

let
  root = ../controllers;
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      (craneLib.fileset.commonCargoSources root)
      # The web app's files are compiled in.
      (root + "/static")
    ];
  };
  commonArgs = {
    inherit src;
    strictDeps = true;
    # Tests run on the build machine, which can't run a cross-compiled binary.
    doCheck = false;
  };
in
craneLib.buildPackage (
  commonArgs
  // {
    cargoArtifacts = craneLib.buildDepsOnly commonArgs;
    meta.mainProgram = "quake-kube-controller";
  }
)
