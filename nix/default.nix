# Everything the flake builds, for one build platform. The flake calls this with
# its inputs, and so does dev.nix, which builds from the working tree.
{
  nixpkgs,
  gomod2nix,
  crane,
  flocken,
  system,
}:

let
  overlays = [ gomod2nix.overlays.default ];
  pkgs = import nixpkgs {
    inherit system overlays;
  };
  inherit (pkgs) lib;

  # Images are cross compiled from this platform for each platform they run on.
  platforms = {
    x86_64 = "x86_64-linux";
    aarch64 = "aarch64-linux";
  };
  pkgsFor =
    target:
    if target == system then
      pkgs
    else
      import nixpkgs {
        localSystem = system;
        crossSystem = target;
        inherit overlays;
      };

  ioquake3 = pkgs.callPackage ./ioquake3.nix { };
  # The same on every platform.
  webClient = pkgs.callPackage ioquake3.webClient { };
  content = {
    urban-terror = pkgs.callPackage ../content/urban-terror { };
    cpma = pkgs.callPackage ../content/cpma { };
    openarena = pkgs.callPackage ../content/openarena { };
    rocket-arena = pkgs.callPackage ../content/rocket-arena { };
    quake3-maps = pkgs.callPackage ../content/quake3-maps { };
    ufreeze = pkgs.callPackage ../content/ufreeze { };
    defrag = pkgs.callPackage ../content/defrag { };
  };

  packagesFor = target: rec {
    pkgs = pkgsFor target;
    q3 = pkgs.callPackage ./q3.nix { };
    ioq3ded = pkgs.callPackage ioquake3.server { };
    # Runs mods' 32-bit x86 game code (see game.native), so only on x86-64.
    ioq3ded-i386 =
      if pkgs.stdenv.hostPlatform.isx86_64 then
        pkgs.pkgsi686Linux.callPackage ioquake3.server { }
      else
        null;
    controller = pkgs.callPackage ./controller.nix { craneLib = crane.mkLib pkgs; };
    images = import ./images.nix {
      inherit
        pkgs
        q3
        ioq3ded
        ioq3ded-i386
        controller
        webClient
        content
        ;
    };
  };
  native = packagesFor system;
  images = lib.mapAttrs (_: target: (packagesFor target).images) platforms;

  # Pushes an image for every platform to ghcr.io as a multi-arch image, e.g.
  #   GITHUB_ACTOR=... GITHUB_TOKEN=... nix run --impure .#manifests.server
  # It's tagged with the version and, on the default branch, latest.
  #
  # flocken's GitHub support is left off: it fetches the repository's details from
  # the GitHub API without a token, which CI's shared addresses get rate limited on.
  # Its branch still comes from GITHUB_REF_NAME.
  manifest =
    image: repo: version:
    flocken.legacyPackages.${system}.mkDockerManifest {
      # Off, but flocken reads the token regardless.
      github.token = "";
      registries."ghcr.io" = {
        enable = true;
        inherit repo;
        username = builtins.getEnv "GITHUB_ACTOR";
        password = builtins.getEnv "GITHUB_TOKEN";
      };
      annotations.org.opencontainers.image = {
        title = lib.last (lib.splitString "/" repo);
        description = "Quake 3 on Kubernetes";
        source = "https://github.com/ChrisRx/quake-kube";
        url = "https://chrisrx.github.io/quake-kube/";
        authors = "https://github.com/ChrisRx";
        vendor = "ChrisRx";
        licenses = "Apache-2.0";
      };
      inherit version;
      images = lib.mapAttrsToList (_: platformImages: platformImages.${image}) images;
    };
in
{
  inherit pkgs images content;

  packages = {
    default = native.q3;
    inherit (native) q3 ioq3ded controller;
    web-client = webClient;
    # The preview on the docs site.
    play-site = pkgs.callPackage ./play-site.nix {
      inherit webClient;
      inherit (content) openarena cpma;
      urbanTerror = content.urban-terror;
    };
    # For hack/play-upload.sh.
    urban-terror-content = content.urban-terror;
    cpma-content = content.cpma;
    # Images for this platform, e.g. for Tilt.
    server-image = native.images.server;
    controller-image = native.images.controller;
    urban-terror-image = native.images.urban-terror;
    cpma-image = native.images.cpma;
    openarena-image = native.images.openarena;
    rocket-arena-image = native.images.rocket-arena;
    quake3-maps-image = native.images.quake3-maps;
    ufreeze-image = native.images.ufreeze;
    defrag-image = native.images.defrag;
  };

  manifests = {
    # Release versions come from the VERSION environment variable, e.g. a git tag.
    server = manifest "server" "chrisrx/quake-kube-server" (builtins.getEnv "VERSION");
    controller = manifest "controller" "chrisrx/quake-kube-controller" (builtins.getEnv "VERSION");
    # Content images are tagged with their mod's version.
    urban-terror = manifest "urban-terror" "chrisrx/quake-kube-urban-terror" content.urban-terror.version;
    cpma = manifest "cpma" "chrisrx/quake-kube-cpma" content.cpma.version;
    openarena = manifest "openarena" "chrisrx/quake-kube-openarena" content.openarena.version;
    rocket-arena = manifest "rocket-arena" "chrisrx/quake-kube-rocket-arena" content.rocket-arena.version;
    quake3-maps = manifest "quake3-maps" "chrisrx/quake-kube-quake3-maps" content.quake3-maps.version;
    ufreeze = manifest "ufreeze" "chrisrx/quake-kube-ufreeze" content.ufreeze.version;
    defrag = manifest "defrag" "chrisrx/quake-kube-defrag" content.defrag.version;
  };
}
