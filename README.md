![Build Status](https://github.com/ChrisRx/quake-kube/workflows/Push%20Image/badge.svg)

<p align="center">
  <img src="./docs/src/images/quake-kube-dark.png" width=305 />
  <h1 align="center">QuakeKube</h1>
</p>

QuakeKube is a Kubernetes-ified version of [Quake 3](https://en.wikipedia.org/wiki/Quake_III_Arena). A controller runs dedicated servers for `QuakeServer` objects, along with a master server that lists them, and a web app creates servers and plays them in the browser with [ioquake3](https://github.com/ioquake/ioq3) built for the web with [Emscripten](https://emscripten.org/). It's installed with a Helm chart.

---

More information on usage can be found in our [Book](https://chrisrx.github.io/quake-kube/).
