# The flake's packages, built from the working tree. A flake only sees the files
# git tracks, which leaves out new ones until they're added. Tilt builds with
# this, e.g.:
#
#   nix build --impure -f nix/dev.nix controller-image
#
# The flake's inputs are fetched at the revisions flake.lock pins, rather than
# with builtins.getFlake on this directory, which would copy the whole working
# tree into the store on every change, build output like controllers/target
# included. The packages read the working tree through filesets instead.
let
  lock = builtins.fromJSON (builtins.readFile ../flake.lock);
  encode = builtins.replaceStrings [ "+" "/" "=" ] [ "%2B" "%2F" "%3D" ];
  input =
    name:
    let
      locked = lock.nodes.${lock.nodes.root.inputs.${name}}.locked;
    in
    builtins.getFlake "github:${locked.owner}/${locked.repo}/${locked.rev}?narHash=${encode locked.narHash}";
in
(import ./. {
  nixpkgs = input "nixpkgs";
  gomod2nix = input "gomod2nix";
  crane = input "crane";
  flocken = input "flocken";
  system = builtins.currentSystem;
}).packages
