# How it works

QuakeKube runs [ioquake3](https://ioquake3.org) dedicated servers on Kubernetes,
and ioquake3 built with [Emscripten](https://emscripten.org/) as the browser
client.

## Controller

The controller reconciles `QuakeServer` objects. For each one it creates:

- a Secret with the server's configuration and a generated rcon password
- a Deployment running the game server
- a Service for the gateway to reach the server inside the cluster
- a Service exposing the game's UDP port to native clients, depending on
  `gameServers.exposure` (LoadBalancer, NodePort, a host port or nothing)

The server's status has its external address once it's reachable.

The controller also runs a Quake 3 master server. Game servers send it
heartbeats, and it answers server browsers' queries with every ready server's
external address.

## Game servers

Each game server's pod runs the `q3` command from the game server image:

- `q3 fetch`, an init container, puts the server's content in place: it
  downloads pk3s and zip archives, and links the files from content images,
  which are mounted as image volumes.
- `q3 run` adds the Quake 3 demo's files, turns the configuration into a
  `server.cfg` and runs `ioq3ded`, restarting it when the configuration
  changes. When the pod stops, it warns players and counts down before
  shutting the server down.

`q3 run` also serves the server's content and info over HTTP, for browser
clients, its Prometheus metrics, and a gRPC health check for the pod's probes,
all on port 8080.

## Gateway

The gateway serves the web app, which lists the servers, creates them from the
presets in `gateway.presets`, and runs the browser client.

The Quake 3 protocol is UDP, which browsers can't send. The browser client's
traffic goes to the gateway instead, which relays it to the server's UDP port.
It uses [WebTransport](https://developer.mozilla.org/en-US/docs/Web/API/WebTransport_API)
when it can, whose unreliable datagrams behave like UDP, and falls back to a
WebSocket, where one lost packet holds up everything behind it. Since the
servers only ever see UDP, they're plain ioquake3 servers that native clients
can join too.

## Browser client

The play page downloads the server's content through the gateway, the same
files the server has, so it plays the server's mod and maps. It keeps them in
the browser's cache, keyed by checksum, so they're only downloaded once and
shared between servers. Mods have to ship QVMs to run in the browser, since it
can't load native game libraries.

## Quake 3 demo EULA

The game servers use the files from the Quake 3 demo, whose End-User License
Agreement must be agreed to before they're distributed. `q3 run` won't start
without `--agree-eula`, which the controller passes, so installing QuakeKube
means agreeing to it. Browser clients show it to players before downloading the
files.
