# The flake's packages, built from the working tree. A flake only sees the files
# git tracks, which leaves out new ones until they're added. Tilt builds with
# this, e.g.:
#
#   nix build --impure -f nix/dev.nix controller-image
let
  flake = builtins.getFlake (toString ../.);
in
(import ./. {
  inherit (flake.inputs)
    nixpkgs
    gomod2nix
    crane
    flocken
    ;
  system = builtins.currentSystem;
}).packages
