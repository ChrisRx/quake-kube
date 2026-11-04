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

Every push to main publishes the images and the chart (see
[Publishing](#publishing)).

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

`task ui` opens it. Tilt's UI also links to it.

Its certificate comes from cert-manager's `local-ca` issuer. For browsers to
trust it, which WebTransport needs, run this once per machine:

```shell
task ui:trust
```

It creates a local CA with [mkcert](https://github.com/FiloSottile/mkcert) and
adds it to Chrome's and Firefox's trust stores (restart them afterward). The
dev cluster then signs with that CA, from `task cluster` or the next `task ui`,
so the web app is trusted, including after recreating the cluster. On NixOS,
mkcert only changes the browsers' trust stores; the system's are declared in
its configuration, e.g. `security.pki.certificateFiles`.

Without mkcert's CA, each cluster has a CA of its own. `task ui` writes it to
`hack/k3d/local-ca.crt` when it changes and says how to import it into a
browser, which a new cluster needs again. `task ui:ca` writes it on its own.

`task ui:forward` opens the web app through a port-forward instead, with the
current kubectl context, which also works for a cluster installed with the
chart, e.g. `task ui:forward NAMESPACE=games PORT=9090`. Without HTTPS, browsers
use a WebSocket rather than WebTransport, and don't cache the game's files.

The dev cluster's presets, in `hack/k3d/presets.yaml`, are the chart's with
content images from the working tree. Tilt builds them and pushes them to the
cluster's registry. The first build downloads the mods, 1.4 GB for Urban Terror, which
nix keeps for later builds. Servers running mods are also in `controllers/example-cpma.yaml` and
`controllers/example-urt.yaml`.

## Profiles

Content images' profiles say which files the browser client reads on each map
(see [Add Mods And Maps](add-custom-maps.md#profiles)). Regenerate one after
changing a mod's content or its preset's maps, with the dev cluster running:

```shell
task content:profile PRESET=urban-terror CONTENT=urban-terror
```

This starts a server for each of the preset's maps in `hack/k3d/presets.yaml`,
joins it in headless Chromium with the engine logging the files it opens, and
writes `content/<mod>/profile.json`, which the content image includes. Files
read on at least half the maps are fetched for every map.

## Publishing

There are no tagged releases. Every push to main pushes the server and
controller images tagged `main` and `latest`, then publishes the chart to
`oci://ghcr.io/chrisrx/quake-kube` with the version in `chart/Chart.yaml`,
replacing the published chart with that version. The chart deploys the
`latest` images. CI adds the commit to the published chart's annotations,
which its pods carry, so `helm upgrade` restarts them and they pull the newest
images. `task chart:push` publishes the chart by hand.

Content images are versioned separately, by the mod's version in
`content/<mod>/default.nix`, and published when `content/` changes.

## Tasks

Common tasks are in `Taskfile.yml`, for [Task](https://taskfile.dev), which
the flake's dev shell has. `task` lists them:

```shell
task test   # Go and Rust tests
task lint   # golangci-lint, clippy, rustfmt and helm lint
task crds   # regenerate the chart's CRDs after changing the controller's types
task docs   # build this book
task docs:site   # and the browser preview at /play, as published
task docs:serve  # build those and serve them at http://127.0.0.1:3000/, press space to open the preview
```
