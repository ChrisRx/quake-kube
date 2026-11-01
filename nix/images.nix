# Container images for one platform. pkgs is the package set for the platform
# the images run on. The web client and content are the same on every platform,
# so they're built once.
{
  pkgs,
  q3,
  ioq3ded,
  controller,
  webClient,
  content,
}:

let
  inherit (pkgs) dockerTools lib;

  # The gateway serves the web client from here.
  client = pkgs.runCommand "quake-kube-client" { } ''
    mkdir -p $out/usr/share/quake-kube
    ln -s ${webClient} $out/usr/share/quake-kube/client
  '';

  # Content images are mounted as image volumes in game server pods, where links into
  # /nix/store wouldn't resolve, so their files are copied in.
  contentImage =
    name: files:
    dockerTools.buildLayeredImage {
      inherit name;
      tag = "latest";
      extraCommands = ''
        cp -r --no-preserve=mode ${files}/. .
      '';
    };
in
{
  server = dockerTools.buildLayeredImage {
    name = "quake-kube-server";
    tag = "latest";
    contents = [
      q3
      ioq3ded
      # q3 fetch downloads content over HTTPS.
      pkgs.cacert
    ];
    config = {
      Entrypoint = [ (lib.getExe q3) ];
      Env = [
        "PATH=/bin"
        "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt"
      ];
    };
  };

  controller = dockerTools.buildLayeredImage {
    name = "quake-kube-controller";
    tag = "latest";
    contents = [ client ];
    config = {
      Entrypoint = [ (lib.getExe controller) ];
      # Numeric so that runAsNonRoot can verify it.
      User = "65532:65532";
    };
  };

  urban-terror = contentImage "quake-kube-urban-terror" content.urban-terror;
  cpma = contentImage "quake-kube-cpma" content.cpma;
}
