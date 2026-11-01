{
  description = "quake-kube flake";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";
  inputs.gomod2nix.url = "github:nix-community/gomod2nix";
  inputs.gomod2nix.inputs.nixpkgs.follows = "nixpkgs";
  inputs.gomod2nix.inputs.flake-utils.follows = "flake-utils";
  inputs.flocken.url = "github:mirkolenz/flocken/v2";
  inputs.flocken.inputs.nixpkgs.follows = "nixpkgs";
  inputs.crane.url = "github:ipetkov/crane";

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      gomod2nix,
      flocken,
      crane,
    }:
    (flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (
      system:
      let
        quakeKube = import ./nix {
          inherit
            nixpkgs
            gomod2nix
            crane
            flocken
            system
            ;
        };
        # Separate from the packages', so the dev shell works without evaluating them.
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ gomod2nix.overlays.default ];
          # needed for terraform
          config.allowUnfree = true;
        };
      in
      {
        # Named here so that listing them, which nix does to resolve any installable,
        # doesn't evaluate the packages.
        packages = nixpkgs.lib.genAttrs [
          "default"
          "q3"
          "ioq3ded"
          "controller"
          "web-client"
          "server-image"
          "controller-image"
          "urban-terror-image"
          "cpma-image"
        ] (name: quakeKube.packages.${name});

        legacyPackages = {
          # Images for each platform, e.g. images.aarch64.server, and their multi-arch
          # manifests for publishing.
          inherit (quakeKube) images manifests;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            pkgs.gomod2nix
            go_1_27
            gopls
            gotools
            go-tools
            mdbook
            go-task
            golangci-lint
            cargo
            rustc
            clippy
            rustfmt
            kubernetes-helm
            k3d
            kind
            kubectl
            tilt
            ctlptl
            terraform
            ioquake3
            skopeo
            gh
          ];
        };
      }
    ));
}
