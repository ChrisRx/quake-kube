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
        };
        # kind with rootless podman. It always uses the user's podman, even
        # when DOCKER_HOST or CONTAINER_HOST point at the rootful socket k3d
        # uses, and creates clusters in a systemd scope with the cgroup
        # controllers delegated, as kind's docs advise for rootless podman.
        kind = pkgs.writeShellScriptBin "kind" ''
          unset DOCKER_HOST CONTAINER_HOST
          export KIND_EXPERIMENTAL_PROVIDER=podman
          if [ "''${1:-}" = create ]; then
            exec systemd-run --user --scope --quiet --property=Delegate=yes ${pkgs.lib.getExe pkgs.kind} "$@"
          fi
          exec ${pkgs.lib.getExe pkgs.kind} "$@"
        '';
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
          "openarena-image"
          "rocket-arena-image"
          "quake3-maps-image"
        ] (name: quakeKube.packages.${name});

        legacyPackages = {
          # Images for each platform, e.g. images.aarch64.server, and their multi-arch
          # manifests for publishing. CI reads the content's versions and downloads.
          inherit (quakeKube) images manifests content;
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
            ioquake3
            # Local certificates the browsers trust, for the dev cluster's
            # web app (task ui:trust). certutil adds them to Chrome's and
            # Firefox's trust stores.
            mkcert
            nss.tools
            skopeo
            gh
          ];
        };
        # What CI's tasks need (lint, test, docs and the chart), without the
        # default shell's cluster tools.
        devShells.ci = pkgs.mkShell {
          buildInputs = with pkgs; [
            go_1_27
            golangci-lint
            cargo
            rustc
            clippy
            rustfmt
            kubernetes-helm
            go-task
            mdbook
          ];
        };
      }
    ));
}
