{
  description = "quake-kube flake";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";
  inputs.gomod2nix.url = "github:nix-community/gomod2nix";
  inputs.gomod2nix.inputs.nixpkgs.follows = "nixpkgs";
  inputs.gomod2nix.inputs.flake-utils.follows = "flake-utils";

  outputs = { self, nixpkgs, flake-utils, gomod2nix }:
    (flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ]
      (system:
        let

          pkgs = import nixpkgs {
            inherit system;

            # needed for terraform 
            config.allowUnfree = true;

            overlays = [
              gomod2nix.overlays.default
            ];
          };

          # Pinning specific packages.
          oldPkgs = import
            (builtins.fetchTarball {
              url = "https://github.com/NixOS/nixpkgs/archive/9957cd48326fe8dbd52fdc50dd2502307f188b0d.tar.gz";
              sha256 = "sha256:1l2hq1n1jl2l64fdcpq3jrfphaz10sd1cpsax3xdya0xgsncgcsi";
            })
            {
              inherit system;
            };


          # The current default sdk for macOS fails to compile go projects, so we use a newer one for now.
          # This has no effect on other platforms.
          callPackage = pkgs.darwin.apple_sdk_11_0.callPackage or pkgs.callPackage;
        in
        rec {
          packages.grpc_health_probe = pkgs.buildGoModule rec {
            pname = "grpc-health-probe";
            version = "0.4.24";

            src = pkgs.fetchFromGitHub {
              owner = "grpc-ecosystem";
              repo = "grpc-health-probe";
              rev = "v${version}";
              sha256 = "sha256-OZ6vfRYO75kaDVrs/HTZCAPuJyoXOk/p4t85JrLrPwQ=";
            };

            ldflags = [
              "-s"
              "-w"
              "-X main.versionTag=v${version}"
            ];

            vendorHash = "sha256-4EJBhdHIRLMHCHThwBItF8ZWVJwU+/enq0AkRcP2Wk4=";

            nativeCheckInputs = with pkgs; [
            ];

            checkPhase = '''';

            meta = with pkgs.lib; {
              description = "A command-line tool to perform health-checks for gRPC applications in Kubernetes and elsewhere";
              homepage = "https://github.com/grpc-ecosystem/grpc-health-probe";
              license = licenses.asl20;
              mainProgram = "grpc-health-probe";
            };
          };
          packages.default = pkgs.buildEnv {
            name = "quake-kube";
            paths = with pkgs; [
              packages.q3
              packages.grpc_health_probe
              ioquake3
            ];
          };
          packages.q3 = pkgs.buildGoApplication {
            pname = "q3";
            version = "0.1";
            subPackages = [
              "./cmd/q3"
            ];
            src = ./.;
            modules = ./gomod2nix.toml;
          };
          packages.container = pkgs.dockerTools.buildLayeredImage {
            name = "quake-kube";
            tag = "latest";
            created = "now";
            contents = [
              packages.default
              packages.grpc_health_probe
              pkgs.ioquake3
            ];
            config.Cmd = [ "${packages.default}/bin/q3" ];
          };

          devShells.default =
            pkgs.mkShell {
              buildInputs = with pkgs; [
                pkgs.gomod2nix
                go_1_21
                gopls
                gotools
                go-tools
                protobuf
                oldPkgs.protoc-gen-go # v1.31.0
                oldPkgs.protoc-gen-go-grpc # v1.3.0
                mdbook
                kubernetes-helm
                kind
                kubectl
                tilt
                ctlptl
                terraform
                ioquake3
                skopeo
                packages.grpc_health_probe
              ];
            };
        })
    );
}
