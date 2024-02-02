# How it works

QuakeKube makes use of [ioquake](https://www.ioquake.org) for the Quake 3 dedicated server, and [QuakeJS](https://github.com/inolen/quakejs), a port of ioquake to javascript using [Emscripten](http://github.com/kripken/emscripten), to provide an in-browser game client.

## Networking

The client/server protocol of Quake 3 uses UDP to synchronize game state. Browsers do not natively support sending UDP packets so QuakeJS wraps the client and dedicated server net code in websockets, allowing the browser-based clients to send messages and enable multiplayer for other clients. This ends up preventing the browser client from using any other Quake 3 dedicated server. In order to use other Quake 3 dedicated servers, a proxy handles websocket traffic coming from browser clients and translates that into UDP to the backend. This gives the flexibility of being able to talk to other existing Quake 3 servers, but also allows using ioquake (instead of the javascript translation of it), which uses *considerably* less CPU and memory.

QuakeKube also uses a cool trick with [cmux](https://github.com/cockroachdb/cmux) to multiplex the client and websocket traffic into the same connection. Having all the traffic go through the same address makes routing a client to its backend much easier (since it can just use its `document.location.host`).

## Quake 3 demo EULA

The Quake 3 dedicated server requires an End-User License Agreement be agreed to by the user before distributing the Quake 3 demo files that are used (maps, textures, etc). To ensure that the installer is aware of, and agrees to, this EULA, the flag `--agree-eula` must be passed to `q3 server` at runtime. This flag is not set by default in the container image and is therefore required for the dedicated server to pass the prompt for EULA. The [example.yaml](example.yaml) manifest demonstrates usage of this flag to agree to the EULA.
