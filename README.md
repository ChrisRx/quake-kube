![Build Status](https://github.com/ChrisRx/quake-kube/workflows/Push%20Image/badge.svg)

<p align="center">
  <img src="./docs/src/images/quake-kube-dark.png" width=305 />
  <h1 align="center">QuakeKube</h1>
</p>

QuakeKube is a Kubernetes-ified version of [Quake 3](https://en.wikipedia.org/wiki/Quake_III_Arena). A controller runs dedicated servers for `QuakeServer` objects, along with a master server that lists them, and a web app creates servers and plays them in the browser with [ioquake3](https://github.com/ioquake/ioq3) built for the web with [Emscripten](https://emscripten.org/). It's installed with a Helm chart.

[Try the web app in your browser](https://q3.chrisrx.dev/try-it.html), with nothing to install.

## Community content

QuakeKube ships mods and community maps made by people in the Quake 3 community, many of them years ago. They're included because their readmes allow distributing them, shipped unmodified with their readmes, and credited to their authors in the [Credits](https://chrisrx.github.io/quake-kube/credits.html#community-maps). They're bundled into QuakeKube's content images, rather than downloaded from community sites like [lvlworld](https://lvlworld.com) when servers start, so running QuakeKube doesn't put load on the sites that have kept these maps available for decades. Many of those authors can't be reached anymore, but their wishes come first: if you made something QuakeKube ships and want it removed, or its credit changed, [open a content removal request](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml) and it will be taken out. See [Content removal](https://chrisrx.github.io/quake-kube/credits.html#content-removal) for more.

---

More information on usage can be found in our [Book](https://chrisrx.github.io/quake-kube/).
