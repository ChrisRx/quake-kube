# Development

## Layout

- `controllers/`: the Rust binary for the controller (including the master
  server) and the gateway, the web app's static files, and the browser client's
  build (upstream ioquake3 with Emscripten, see `controllers/client/`)
- `cmd/q3`, `internal/`: the Go `q3` command that runs in each game
  server's pod (`q3 fetch` and `q3 run`)
- `chart/`: the Helm chart. `chart/files/crds` is generated from the Rust types
  with `cargo run -- crdgen`.
- `content/`: content images for mods (see [Add Mods And Maps](add-custom-maps.md))
- `nix/`: how the flake builds everything, including the images
- `hack/`: the local cluster's setup

## Images

Releases are `v*` tags. The `Push Image` workflow pushes the images tagged
with the release's version, then the Helm chart, versioned to match, to
`oci://ghcr.io/chrisrx/quake-kube`. `task chart:push VERSION=1.2.0` does the
same for the chart by hand.

The flake builds every image, cross compiling from the build machine for
x86_64 and aarch64. The ones for the build machine's platform are packages:

```shell
nix build .#controller-image   # also server-image, urban-terror-image, cpma-image
docker load < result
```

and every platform's are in `images`, e.g. `.#images.aarch64.server`. The
`manifests` push them to ghcr.io as multi-arch images, which the
`Push Image` and `Push Content Images` workflows run:

```shell
GITHUB_TOKEN=... VERSION=v1.0.0 nix run --impure .#manifests.controller
```

A flake only sees the files git tracks, so new files are left out until
they're added. `nix/dev.nix` builds the same packages from the working tree,
which is what Tilt uses:

```shell
nix build --impure -f nix/dev.nix controller-image
```

## Local cluster

Development uses a [k3d](https://k3d.io/) cluster that mirrors a typical cloud
setup: Traefik for ingress, [MetalLB](https://metallb.io/) for LoadBalancer
Services and [cert-manager](https://cert-manager.io/) with a local CA. It
requires k3d 5.8 or newer, which the flake's dev shell provides.

```shell
task cluster
```

This creates the `k3d-quake-kube` context and a registry at `localhost:5005`,
and prints the Traefik address. LoadBalancer addresses come from
`172.28.0.100-172.28.0.199` and are reachable directly from the host on Linux.
Set `K3D_SUBNET` to a different `/24` if `172.28.0.0/24` is already in use.

With podman, the rootful socket (`/run/podman/podman.sock`) is used
automatically, since rootless podman networks aren't reachable from the host.

`task cluster-delete` removes the cluster and registry.

## Tilt

[Tilt](https://tilt.dev/) builds the controller and game server images with
nix, deploys the chart and the example server in `controllers/example.yaml`,
and rebuilds as files change:

```shell
task dev
```

This creates the cluster if needed. The web app is at
`https://quake.172-28-0-100.sslip.io` through Traefik.

Its certificate comes from the cluster's local CA. To open it without
warnings, write the CA certificate with `hack/dev.sh ca` and import it into
your browser as a trusted authority. The CA is new each time the cluster is
created.

The dev cluster's presets, in `hack/k3d/presets.yaml`, include Urban Terror
and CPMA. Tilt builds their content images and pushes them to the cluster's
registry. The first build downloads the mods, 1.4 GB for Urban Terror, which
nix keeps for later builds. Servers running mods are also in `controllers/example-cpma.yaml` and
`controllers/example-urt.yaml`.

## Tasks

Common tasks are in `Taskfile.yml`, for [Task](https://taskfile.dev), which
the flake's dev shell has. `task` lists them:

```shell
task test   # Go and Rust tests
task lint   # golangci-lint, clippy, rustfmt and helm lint
task crds   # regenerate the chart's CRDs after changing the controller's types
task docs   # build this book
```
