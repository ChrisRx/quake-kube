# The q3 command that runs in each game server's pod.
{ lib, buildGoApplication, go_1_27 }:

buildGoApplication {
  pname = "q3";
  version = "0.1";
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../go.mod
      ../go.sum
      ../cmd
      ../internal
    ];
  };
  modules = ../gomod2nix.toml;
  subPackages = [ "./cmd/q3" ];
  go = go_1_27;
  CGO_ENABLED = 0;
  meta.mainProgram = "q3";
}
