<p align="center">
  <img src="./images/quake-kube-dark.png" width=305 />
  <h1 align="center">QuakeKube</h1>
<p>

QuakeKube is a Kubernetes-ified version of [Quake 3](https://en.wikipedia.org/wiki/Quake_III_Arena) that manages a dedicated server in a Kubernetes Deployment. It uses [QuakeJS](https://github.com/inolen/quakejs) to enable clients to connect and play in the browser.

## Limitations

This uses files from the Quake 3 Demo. The demo doesn't allow custom games, so while you can add new maps, you couldn't say load up [Urban Terror](https://www.moddb.com/mods/urban-terror). I think with pak files from a full version of the game this would be possible, but I haven't tried it (maybe one day).

Another caveat is that the copy running in the browser is using [QuakeJS](https://github.com/inolen/quakejs). This version is an older verison of [ioquake3](https://github.com/ioquake/ioq3) built with [emscripten](https://emscripten.org/) and it does not appear to be supported, nor does it still compile with any newer versions of emscripten. I believe this could be made to work again, but I haven't personally looked at how involved it would be. It is worth noting that any non-browser versions of the Quake 3 could connect to the dedicated servers.

## What is this project for?

This was just made for fun and learning. It isn't trying to be a complete solution for managing Quake 3 on Kubernetes, and I am using it now as a repo of common patterns and best practices (IMO) for Go/Kubernetes projects. I think some fun additions though might be adding code to work as a Quake 3 master server, a server that exchanges information with the game client about what dedicated game servers are available, and making a controller/crd that ties it all together.
