![Build Status](https://github.com/ChrisRx/quake-kube/workflows/Push%20Image/badge.svg)

<p align="center">
  <img src="./docs/src/images/quake-kube-dark.png" width=305 />
  <h1 align="center">QuakeKube</h1>
</p>

QuakeKube is a Kubernetes-ified version of [Quake 3](https://en.wikipedia.org/wiki/Quake_III_Arena). A controller runs dedicated servers for `QuakeServer` objects, along with a master server that lists them, and a web app creates servers and plays them in the browser with [ioquake3](https://github.com/ioquake/ioq3) built for the web with [Emscripten](https://emscripten.org/). It's installed with a Helm chart.

[Try the web app in your browser](https://q3.chrisrx.dev/try-it.html), with nothing to install.

## Features

- Create servers from a web app, or as `QuakeServer` objects with `kubectl`
- Play in the browser, or join from any native Quake 3 client
- Offers Quake 3 demo and community maps, OpenArena, Urban Terror, CPMA, Rocket Arena, Ultra Freeze Tag and DeFRaG, and runs other mods from content images or downloads
- Map rotations, bots, passwords and match settings, changed while people play
- Sign-in through a proxy like Google Cloud's Identity-Aware Proxy, with server owners and admins

## Install

```shell
helm install quake-kube oci://ghcr.io/chrisrx/quake-kube \
  --namespace quake-kube --create-namespace
kubectl --namespace quake-kube port-forward service/quake-kube-gateway 8080:80
```

Then open `http://localhost:8080` and create a server. The game servers use the files from the Quake 3 demo, which are covered by its [EULA](internal/quake/server/EULA.txt). See [Getting Started](https://q3.chrisrx.dev/getting-started.html) for more.

## Documentation

The [QuakeKube Book](https://q3.chrisrx.dev/) has the rest:

- [Using the web app](https://q3.chrisrx.dev/web-app.html)
- [How it works](https://q3.chrisrx.dev/how-it-works.html)
- [Configuration](https://q3.chrisrx.dev/configuration.html) and the [QuakeServer reference](https://q3.chrisrx.dev/quakeserver-reference.html)
- [Troubleshooting](https://q3.chrisrx.dev/troubleshooting.html)
- [Development](https://q3.chrisrx.dev/development.html)

## Community content

QuakeKube ships mods and community maps made by people in the Quake 3 community, many of them years ago. They're included because their readmes allow distributing them, shipped unmodified with their readmes, and credited to their authors in the [Credits](https://q3.chrisrx.dev/credits.html#community-maps). They're bundled into QuakeKube's content images, rather than downloaded from community sites like [lvlworld](https://lvlworld.com) when servers start, so running QuakeKube doesn't put load on the sites that have kept these maps available for decades. Many of those authors can't be reached anymore, but their wishes come first: if you made something QuakeKube ships and want it removed, or its credit changed, [open a content removal request](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml) and it will be taken out. See [Content removal](https://q3.chrisrx.dev/credits.html#content-removal) for more.

## License

QuakeKube's code is licensed under the [Apache License 2.0](LICENSE). Game content has licenses of its own (see the [Credits](https://q3.chrisrx.dev/credits.html)).
