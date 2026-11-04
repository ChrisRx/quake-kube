# Monitoring And Troubleshooting

## Logs

The chart's two deployments log to stdout. `controller.logLevel` and
`gateway.logLevel` take `RUST_LOG` filters, e.g.
`info,quake_kube_controller=debug`.

```shell
kubectl --namespace quake-kube logs deployment/quake-kube-controller
kubectl --namespace quake-kube logs deployment/quake-kube-gateway
```

Each game server's Deployment has the server's name. The `server` container has
the game's console output, and the `content` init container, when the server has
content, says what it downloaded:

```shell
kubectl logs deployment/<server> --container server
kubectl logs deployment/<server> --container content
```

## Metrics

Each game server serves Prometheus metrics on port 8080, at `/metrics`, along with
Go's runtime metrics:

| Metric | Type | |
|---|---|---|
| `quake_active_players` | Gauge | Players on the server, including bots |
| `quake_player_scores` | Gauge | Each player's score, with `player` and `map` labels |
| `quake_player_pings` | Gauge | Each player's ping, with a `player` label |
| `quake_config_reloads` | Counter | How many times the server's configuration changed |

The player metrics keep a player's last value until the pod is replaced, so they
still have players who've left. The controller and gateway don't have metrics.

Game servers' Services can be found by their labels. With the Prometheus
Operator:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: quake-servers
spec:
  namespaceSelector:
    any: true
  selector:
    matchLabels:
      app.kubernetes.io/name: quake-server
  endpoints:
    - port: http
```

## Health checks

Game server pods have gRPC health checks on port 8080, and the gateway has
`/healthz`.

## Servers that don't start

Start with the server's status, which says why it's stuck when it can tell. The
web app shows the same message:

```shell
kubectl get quakeserver <server> -o jsonpath='{.status.message}'
```

| The message says | What to look at |
|---|---|
| `the server can't be scheduled` | The nodes' resources, or their architecture: `game.native` needs an x86-64 node. With `HostPort`, the port may already be in use on every node, which a port range fixes (see [Port ranges](networking.md#port-ranges)). |
| `pulling an image failed` | The image's name and tag, and `imagePullSecrets`. Content images are named in the message. |
| `downloading the server's content failed` | The `url` or `sha256` in `content`. The message ends with the end of the container's log. |
| `the server failed` or `the server keeps exiting` | The `server` container's log, which the message ends with when it can. |
| `starting the server failed` | The pod's events, with `kubectl describe pod`. |

With no message, and `ready` still false, the server is usually still starting,
since content downloads and first-time image pulls can take a while. If it stays
that way, the controller's log has any errors from reconciling it, as
`reconcile failed`.

Content images need image volumes, on Kubernetes 1.35 or 1.33 with the
`ImageVolume` feature gate. On older clusters the chart hides the presets that
use them, but a QuakeServer applied with `kubectl` can still ask for them.

## The web app

| What you see | Why |
|---|---|
| **Play** is grayed out on a ready server | The gateway hasn't heard from the server yet, which takes a few seconds. If it lasts, check the server's log. |
| A server disappeared | Servers created from the web app are deleted when idle (see [Idle servers](web-app.md#idle-servers)). The gateway's log has `deleting idle server`. |
| No **Edit** or **Delete** | The server was created with `kubectl`, or belongs to someone else (see [Who can manage a server](web-app.md#who-can-manage-a-server)). |
| No **Create server** button | `gateway.maxServers` is 0. |
| `the limit of N servers has been reached` | `gateway.maxServers` counts every server created from the web app. Delete one, or raise it. |
| `each person can have up to N servers at once` | `gateway.maxServersPerUser`, which applies to admins too. |
| `not signed in` | The gateway didn't get a valid identity from the proxy in front of it, such as a request that skipped the proxy (see [Authentication](authentication.md)). |
| Only some games are offered | The cluster is too old for content images (see [Kubernetes versions](games.md#kubernetes-versions)). |
| Map pictures stay blank | The gateway runs a `quake-levelshots` pod for each game to make them, which gives up after 15 minutes. Check it with `kubectl --namespace quake-kube get pods -l app.kubernetes.io/name=quake-levelshots`, in the web app's namespace. |

## Playing in the browser

| What you see | Why |
|---|---|
| The game stutters when packets are lost | The browser is using a WebSocket. It only uses WebTransport from HTTPS pages with `gateway.webtransport.enabled`. |
| The console says `WebTransport unavailable, using a WebSocket` | Browsers can't reach the gateway's UDP port within 3 seconds, so check `gateway.webtransport.publicAddress`, and that UDP is allowed through firewalls (see [Publish the web app](getting-started.md#publish-the-web-app)). |
| WebTransport fails with several gateway replicas | Set `gateway.webtransport.tls.secretName` and `gateway.auth.ticketSecretName`, since each replica otherwise has its own certificate and key. |
| Players are disconnected every 30 seconds on GKE | GKE's load balancers close WebSockets after 30 seconds without `gateway.gke.enabled`. Other load balancers and ingresses have similar idle timeouts of their own. |
| Files download again on every join | Browsers only cache them on HTTPS pages and on `localhost`. |
| The page runs out of memory | Browsers without JSPI hold a game's paks in memory, so large games need a desktop browser, or one with JSPI like Chrome or Edge. |
| Gray grid surfaces, blank surfaces or silence | A map uses files from the full game (see [Content that needs the full game](add-custom-maps.md#content-that-needs-the-full-game)). |

## Bots

| What you see | Why |
|---|---|
| No bots join, or they don't move | The game may not have any (see [Games](games.md)), or the map has no `.aas` navigation file, which bots need. |
| A bot from `commands` isn't there | `commands` only run when the server starts, and the Quake 3 demo only has some of the game's bots. |

## Native clients

| What you see | Why |
|---|---|
| A server has no address | Its Service is waiting for a load balancer, which `kubectl get service <server>-game` shows, or `gameServers.exposure.type` is `None` (see [Exposing Game Servers](networking.md)). |
| A client can't connect to the address | UDP may be blocked by a firewall or security group. |
| A server isn't in the server browser | It needs to be ready and have an address, the client needs `sv_master1` set to the master server, and the client has to ask for empty servers if nobody's on it (see [The master server](networking.md#the-master-server)). |
| A client drops right after connecting, or complains about missing files | The client may not have the server's mod or maps, which native clients don't get from the gateway (see [Joining from a native client](networking.md#joining-from-a-native-client)). |

## Stopping a server

When a server's pod stops, `q3 run` warns players, counts down for 10 seconds, says
goodbye and kicks everyone before stopping. A pod replaced by a change to its
content or mod does the same.
