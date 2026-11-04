# How it works

QuakeKube runs [ioquake3](https://ioquake3.org) dedicated servers on Kubernetes,
and ioquake3 built with [Emscripten](https://emscripten.org/) as the browser
client. It's made of a few parts:

```pikchr
# Inside the cluster: the gateway and game servers carry the game, and the
# controller turns QuakeServers into running servers.
boxrad = 0.06
$sm = 0.8

GW: box "Gateway" bold "web app, relay" fit wid 1.5 ht 0.62 at (2.9, 0)
POD: box wid 1.9 ht 1.9 at (6.0, -0.55)
text "Game server pod" bold at POD.n + (0, -0.22)
text "q3 run" italic at POD.n + (0, -0.55)
text "files, info, config, HTTP" small at POD.n + (0, -0.75)
text "ioq3ded" italic at POD.n + (0, -1.05)
text "the game, UDP" small at POD.n + (0, -1.25)
text "q3 fetch" italic at POD.n + (0, -1.55)
text "puts the content in place" small at POD.n + (0, -1.75)

API: cylinder "Kubernetes API" bold "QuakeServers" small fit wid 1.5 ht 0.75 at (2.9, -1.55)
CTL: box wid 4.75 ht 0.95 dashed at (4.425, -3.0)
text "Controller" bold at CTL.n + (0, -0.17)
REC: box "Reconciler" bold "QuakeServers into pods" small fit wid 1.5 ht 0.5 at (2.9, -3.1)
MS: box "Master server" bold "lists servers" small fit wid 1.5 ht 0.5 at (6.0, -3.1)

# The cluster's boundary, with what's outside it on either side.
CL: box wid 5.1 ht 4.45 dotted at (4.57, -1.38)
text "Kubernetes cluster" italic ljust at CL.nw + (0.12, -0.16)

# Outside the cluster.
BR: box "Browser" bold "web app, browser client" small fit wid 1.6 ht 0.62 at (0, 0)
NC: box "Native client" bold "ioquake3" small fit wid 1.4 ht 0.62 at (9.1, -0.9)
CN: cylinder "Content" bold "images, downloads" small fit wid 1.4 ht 0.75 at (9.1, 0.35)

# The game, and what browsers download.
arrow from BR.e + (0, 0.12) to GW.w + (0, 0.12) "HTTPS" above
arrow from BR.e + (0, -0.12) to GW.w + (0, -0.12) "WebTransport" below "or WebSocket" below
arrow from GW.e + (0, 0.12) to (POD.w.x, GW.e.y + 0.12) "HTTP" above
arrow from GW.e + (0, -0.12) to (POD.w.x, GW.e.y - 0.12) "UDP" below
arrow from NC.w to (POD.e.x, NC.w.y) "UDP" above
arrow from CN.w to (POD.e.x, CN.w.y) "q3 fetch" above

# Servers and their objects.
arrow from GW.s to API.n
text "creates, changes" ljust at last arrow + (0.1, 0)
arrow from REC.n to API.s <->
text "watches, applies" ljust at last arrow + (0.1, 0)
arrow from API.e + (0, -0.1) to (POD.w.x, API.e.y - 0.1) "Deployment," above "Services, Secret" below dashed
arrow from POD.s to MS.n
text "heartbeats" ljust at last arrow + (0.1, 0)
arrow from NC.s down until even with MS then to MS.e
text "server list" at (MS.e.x + 1.4, MS.e.y + 0.14)
```

- The **gateway** serves the web app and the browser client, and carries
  browsers' game traffic to the servers, since browsers can't send UDP.
- A **QuakeServer** is a game server's spec, stored in the Kubernetes API. The
  web app creates them, and so can `kubectl`.
- The **controller** turns each QuakeServer into a running **game server pod**,
  and runs the master server native clients find servers through.
- **Content**, a mod and its maps, comes from content images or downloads, and
  browsers download the same files from the server.

## Controller

The controller reconciles `QuakeServer` objects. For each one it creates:

- a Secret with the server's configuration and a generated rcon password
- a Deployment running the game server
- a Service for the gateway to reach the server inside the cluster
- a Service exposing the game's UDP port to native clients, depending on
  `gameServers.exposure` (LoadBalancer, NodePort, a host port or nothing)

The server's status says when it's ready, which is once its pod runs the current
spec, and has its external address once it's reachable. When the pod is stuck
rather than starting, e.g. because its image can't be pulled or its content
fails to download, the status has a message saying why, which the web app
shows.

The controller also runs a Quake 3 master server, which answers server browsers'
queries with every ready server's external address. It knows the servers from
their QuakeServers rather than their heartbeats, since heartbeats come from
addresses inside the cluster, so a heartbeat only tells it to check for changes
sooner.

The [QuakeServer Reference](quakeserver-reference.md#what-the-controller-creates)
has the details of what it creates.

## Game servers

Each game server's pod runs the `q3` command from the game server image:

- `q3 fetch`, an init container, puts the server's content in place: it
  downloads pk3s and zip archives, and links the files from content images,
  which are mounted as image volumes.
- `q3 run` adds the Quake 3 demo's files, turns the configuration into a
  `server.cfg` and runs `ioq3ded`. When the pod stops, it warns players and
  counts down before shutting the server down.

`q3 run` also serves, on port 8080, the server's content and info for browser
clients, its maps' levelshots, its [Prometheus metrics](troubleshooting.md#metrics),
and a gRPC health check for the pod's probes.

When the configuration changes, `q3 run` changes the running server from its
console where it can: a different map, game type, limits or bots reload the map,
the way a map rotation does, and players stay connected. Only a different mod
restarts the server, and that replaces its pod anyway.

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

### Changing a server

A server's owner can change it from the web app's server list, or from the game's
pause menu while playing: its map, game type, bots and limits, or which game it
runs. The change is saved in the QuakeServer, but that takes a minute to reach
the server: the controller renders the Secret, and Kubernetes updates the file
in the pod. So the gateway also sends the server the same configuration right
away:

```pikchr
# An owner's change, applied right away, then saved where a restart finds it.
boxrad = 0.06
BR: box "Browser" bold "server settings" small fit wid 1.4 ht 0.6 at (0, 0)
GW: box "Gateway" bold fit wid 1.4 ht 0.6 at (2.7, 0)
RUN: box "q3 run" bold "game server pod" small fit wid 1.4 ht 0.6 at (5.4, 0)
DED: box "ioq3ded" bold "reloads the map" small fit wid 1.4 ht 0.6 at (8.1, 0)
API: cylinder "Kubernetes API" bold "QuakeServer" small fit wid 1.5 ht 0.75 at (2.7, -1.45)
CTL: box "Controller" bold fit wid 1.4 ht 0.6 at (2.7, -2.8)
SEC: box "Secret" bold "config.yaml" small fit wid 1.4 ht 0.6 at (5.4, -2.8)

arrow from BR.e to GW.w "changes" above
arrow from GW.e to RUN.w "2. the new config," above "rcon password" below
arrow from RUN.e to DED.w "3. runs it" above "from the console" below
arrow from GW.s to API.n
text "1. saves the spec" ljust at last arrow + (0.1, 0)
arrow from API.s to CTL.n
text "watches" ljust at last arrow + (0.1, 0)
arrow from CTL.e to SEC.w "renders" above
arrow from SEC.n to RUN.s dashed
text "4. the same config," ljust "a minute later, ignored" ljust at last arrow + (0.1, 0)
```

The gateway renders the configuration with the controller's code and authorizes
it with the server's rcon password, which it reads from the server's Secret.
When the Secret's copy arrives, `q3 run` finds nothing new and ignores it.

Switching to another game changes the server's content, so its pod is replaced.
Players' pages notice when the server comes back running the other game and
rejoin it. The browser client also rejoins when the server changes to a map
whose files it didn't download, unless it reads files as it needs them.

### Idle servers

Servers created from the web app are deleted after nobody has played on them for
`gateway.idleTimeout`, 30 minutes by default. That includes servers that never
start. Kubernetes deletes their pods and Services with them.

### Levelshots

Servers play a rotation of maps, moving to the next one after each match. The web
app shows a picture of each map, its levelshot, which comes from the map's pak,
with its title and the game types it's made for, from its arena file. For maps of
games no server is running, the gateway runs a pod for each preset that assembles
the preset's content like a game server's pod, with `q3 levelshots` serving
thumbnails of the maps' levelshots and what their arena files say about them. The
gateway keeps them in a ConfigMap, `quake-kube-levelshots-<preset>`, and only makes
them again when the preset's content or maps change.

## Browser client

The play page downloads the server's content through the gateway, the same
files the server has, so it plays the server's mod and maps. It keeps them in
the browser's cache, keyed by checksum, so they're only downloaded once and
shared between servers. Browsers with
[JSPI](https://v8.dev/blog/jspi) read the paks as the game needs them instead,
so joining doesn't wait for a whole mod to download. Mods have to ship QVMs to
run in the browser, since it can't load native game libraries.

## Quake 3 demo EULA

The game servers use the files from the Quake 3 demo, whose End-User License
Agreement must be agreed to before they're distributed. `q3 run` won't start
without `--agree-eula`, which the controller passes, so installing QuakeKube
means agreeing to it. Browser clients show it to players before downloading the
files.
