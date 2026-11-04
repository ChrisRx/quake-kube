<p align="center">
  <img src="./images/quake-kube-dark.png" width=305 />
  <h1 align="center">QuakeKube</h1>
</p>

QuakeKube is a Kubernetes-ified version of [Quake 3](https://en.wikipedia.org/wiki/Quake_III_Arena). A controller runs dedicated servers for `QuakeServer` objects, along with a master server that lists them, and a web app creates servers and plays them in the browser with [ioquake3](https://github.com/ioquake/ioq3) built for the web with [Emscripten](https://emscripten.org/). It's installed with a Helm chart.

[Try the web app in your browser](try-it.md), with nothing to install.

## Limitations

This uses files from the Quake 3 Demo, which has four maps. Servers can download more maps and mods, such as [CPMA](https://playmorepromode.com) or [Urban Terror](https://www.urbanterror.info), when they start (see [Add Mods And Maps](add-custom-maps.md)). Mods have to ship QVMs to run in the browser, since it can't load native game libraries.

Any non-browser Quake 3 client can connect to the servers too.

## What is this project for?

This was just made for fun and learning. It isn't trying to be a complete solution for managing Quake 3 on Kubernetes, and I am using it now as a repo of common patterns and best practices (IMO) for Go/Kubernetes projects. It has since grown a Quake 3 master server, a server that exchanges information with the game client about what dedicated game servers are available, and a controller/CRD that ties it all together.
