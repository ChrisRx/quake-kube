# Development

## Layout

- `controllers/`: the Rust binary for the controller (including the master
  server) and the gateway, the web app's static files, and the browser client's
  build (upstream ioquake3 with Emscripten, see `controllers/client/`)
- `cmd/q3`, `internal/`: the Go `q3` command that game server pods run (see
  [The q3 command](#the-q3-command))
- `chart/`: the Helm chart. `chart/files/crds` is generated from the Rust types
  with `cargo run -- crdgen`.
- `content/`: content images for mods (see [Add Mods And Maps](add-custom-maps.md))
- `nix/`: how the flake builds everything, including the images
- `hack/`: the local cluster's setup, and tools for content and the docs site
- `docs/`: this book

The flake's dev shell has the tools everything below uses. Enter it with
`nix develop`, or `direnv allow` with the repo's `.envrc`.

## Images

Every push to main publishes the images and the chart (see
[Publishing](#publishing)).

The flake builds every image, cross compiling from the build machine for
x86_64 and aarch64. The ones for the build machine's platform are packages:

```shell
nix build .#controller-image   # server-image, and <content>-image, e.g. cpma-image
docker load < result
```

and every platform's are in `images`, e.g. `.#images.aarch64.server`. The
`manifests` push them to ghcr.io as multi-arch images, which the
`Push Image` and `Push Content Images` workflows run:

```shell
GITHUB_ACTOR=... GITHUB_TOKEN=... VERSION=v1.0.0 nix run --impure .#manifests.controller
```

A flake only sees the files git tracks, so new files are left out until
they're added. `nix/dev.nix` builds the same packages from the working tree,
which is what Tilt uses:

```shell
nix build --impure -f nix/dev.nix controller-image
```

Never evaluate `builtins.getFlake (toString ./.)` on the repo, in `nix/dev.nix` or
in an ad-hoc `nix build --expr`. It copies the whole working tree into the store
each time it changes, including `controllers/target`, which can fill a disk.
`nix/dev.nix` gets the flake's inputs from `flake.lock` instead.

## The q3 command

`q3` has three commands, which run in pods the controller and gateway create:

| Command | |
|---|---|
| `q3 fetch` | Puts a server's content in place, as its pod's init container. |
| `q3 run` | Runs `ioq3ded` with a configuration, serving content, info, metrics and health checks on port 8080. |
| `q3 levelshots` | Serves thumbnails and details of a preset's maps for the gateway, then exits. |

`q3 run` also works outside a cluster, with `ioq3ded` from the dev shell on the
`PATH`. It extracts the demo's files into `--assets-dir`, and without `--config`
plays the [default maps and settings](quakeserver-reference.md):

```shell
task build
bin/q3 run --agree-eula --assets-dir /tmp/q3
```

`--config` takes a file like the `config.yaml` in a server's Secret, which `q3 run`
checks for changes every `--watch-interval`.

It stops gracefully on SIGTERM, which is how Kubernetes stops it: it warns players,
counts down for `--shutdown-delay` and kicks them, and a second signal stops it
immediately. In a terminal, the first CTRL+C only asks for another within 5 seconds
to confirm, the next starts that graceful shutdown, and another stops it immediately.

## The browser client

The browser client is upstream ioquake3 built with Emscripten by
`nix/ioquake3.nix`, at the commit pinned there, with the patches in
`controllers/client/patches`:

- `0001-net-skip-fionbio-under-emscripten.patch` keeps networking working, since
  Emscripten's sockets fail the call that makes them non-blocking.
- `0002-web-wait-for-pak-reads-with-jspi.patch` adds the build that reads paks as
  the game needs them, which the gateway serves at `/client/jspi/`.
- `0003-skip-id-authorize-server.patch` skips id's authorize server, which is gone.

The gateway embeds the web app's files from `controllers/static`, and serves the
client from the controller image.

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
chart, e.g. `task ui:forward NAMESPACE=games PORT=9090`. A port-forward only
carries TCP, so browsers use a WebSocket rather than WebTransport.

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

## Adding content

To add a map or mod to a content image:

1. Add it to `content/<mod>/default.nix`, with its download pinned by hash, keeping
   its files unmodified and its readme with it.
2. Bump `version` there, and the image's tag in `chart/values.yaml` and
   `hack/k3d/presets.yaml`. Published tags aren't overwritten, so without a new
   version CI skips the image.
3. Add its maps to their presets in both of those files.
4. Add it to the [Credits](credits.md), with its author. Community maps get a
   **Request removal** link like the others in that table.
5. Regenerate the stand-ins' list if it uses the full game's textures (see
   [Content that needs the full game](add-custom-maps.md#content-that-needs-the-full-game)),
   and the content's [profile](#profiles).

Decisions about the Quake 3 community maps' textures are recorded in
`content/quake3-maps/standins/`, whose
[README](https://github.com/ChrisRx/quake-kube/blob/main/content/quake3-maps/standins/README.md)
covers `task content:sweep`, the picker for revisiting them.

To add a new content image, add its directory to `content/`, to
`nix/default.nix`, to the `Push Content Images` workflow's matrix, and to the
[table of images](add-custom-maps.md#content-images).

## The docs site

This book is built with [mdBook](https://rust-lang.github.io/mdBook/) from
`docs/`. Its diagrams are [pikchr](https://pikchr.org/) blocks, drawn by
`hack/mdbook-pikchr`.

The site also has the [browser preview](try-it.md) at `/play/`: the real web app,
with `controllers/static/preview.js` answering its requests in place of a gateway.
`hack/play-site` generates it and `nix/play-site.nix` packages it.

The preview's games are too large for GitHub Pages, which also doesn't reliably
answer the Range requests they're read with, so their files are hosted on R2. They
only change with a mod's version, so they're uploaded by hand rather than from CI,
with `task docs:upload`, which reads the bucket's credentials from
`~/.config/quake-kube/r2.env`.

## Publishing

There are no tagged releases. Every push to main pushes the server and
controller images tagged `main` and `latest`, then publishes the chart to
`oci://ghcr.io/chrisrx/quake-kube` with the version in `chart/Chart.yaml`,
replacing the published chart with that version. The chart deploys the
`latest` images. CI adds the commit to the published chart's annotations,
which its pods carry, so `helm upgrade` restarts them and they pull the newest
images. `task chart:push` publishes the chart by hand.

Content images are versioned separately, by the mod's version in
`content/<mod>/default.nix`. They're published when `content/` changes, skipping
any whose version is already published.

The workflows in `.github/workflows`:

| Workflow | Runs on | |
|---|---|---|
| `Checks` | Pushes to main, and pull requests | `task lint test`, `nix flake check`, and builds the server and controller images |
| `Push Image` | Pushes to main | Publishes the server and controller images and the chart |
| `Push Content Images` | Changes to `content/` on main | Publishes content images with new versions |
| `Deploy static content to Pages` | Pushes to main | Publishes this book and the preview |

Builds are cached in the `quake-kube` Cachix cache, leaving out content and images.

## Contributing

Run `task lint test` before sending a change, which is what CI runs. After changing
the controller's types, run `task crds` and include the result. Commit messages
follow [Conventional Commits](https://www.conventionalcommits.org/), like
`fix(web): stop using pak ranges cached from GitHub Pages`.

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
task docs:upload # upload the preview's game files to R2, after a mod's version changes
task content:profile PRESET=cpma CONTENT=cpma  # regenerate a content image's profile
```
