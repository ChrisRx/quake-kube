# Exposing Game Servers

Browser clients only need to reach the gateway, which relays their game traffic
to the servers. Exposing game servers is only needed for native clients, which
connect to them directly over UDP.

## Ports

| What | Default port | Protocol | Service | Who needs it |
|---|---|---|---|---|
| Web app, and the WebSocket relay | 80 (`gateway.service.port`) | TCP | `<release>-gateway` | Browsers, usually through an ingress |
| WebTransport relay | 443 (`gateway.webtransport.service.port`) | UDP | `<release>-gateway-webtransport` | Browsers, when it's turned on |
| Master server | 27950 (`master.port`) | UDP | `<release>-master` | Native clients' server browsers |
| Game | 27960, or from `portRange` | UDP | `<server>-game` | Native clients |
| Content, info and metrics | 8080 | TCP | `<server>` | The gateway and Prometheus, inside the cluster |

Check that firewalls and security groups allow the UDP ports.

## Exposure types

`gameServers.exposure.type` sets how native clients reach every game server:

| Type | What the controller creates | The server's address |
|---|---|---|
| `LoadBalancer`, the default | A UDP LoadBalancer Service, `<server>-game` | The load balancer's address |
| `NodePort` | A UDP NodePort Service, `<server>-game` | The node's address and the node port |
| `HostPort` | A host port on the server's pod, with no Service | The node's address and the port |
| `None` | Nothing | None, so only browsers can join |

For `NodePort` and `HostPort`, the address is the node's external IP if it has
one, and otherwise its internal IP.

A server's address is in its status, and in the web app:

```shell
kubectl get quakeservers --all-namespaces
```

Servers are ready as soon as they're running, so browsers can join a server whose
address hasn't arrived yet. On a cluster without load balancers, a `LoadBalancer`
Service waits for an address forever, and the server still plays in the browser.

## Port ranges

Without a range, every server's external port is 27960, or a NodePort chosen by
Kubernetes. That works when each server gets its own load balancer address, but
not when they share one, and with `HostPort` only one server fits on each node.

`gameServers.exposure.portRange` gives each server its own port:

```yaml
gameServers:
  exposure:
    type: HostPort
    portRange: "27960-27999"
```

The controller records each server's port in its status, so it survives restarts,
and keeps the ports unique across all namespaces. With `NodePort`, the range has
to be inside the cluster's NodePort range, 30000-32767 by default. When a range
runs out, new servers don't start, and their status says so.

## Sharing an address

Giving every server a load balancer can be expensive, or impossible with few
addresses. With [MetalLB](https://metallb.io/), servers can share one address
with a port range:

```yaml
gameServers:
  exposure:
    annotations:
      metallb.io/allow-shared-ip: quake-kube
      metallb.io/loadBalancerIPs: 172.28.0.150
    portRange: "27960-27999"
```

`gameServers.exposure.annotations` is added to each server's Service, which is
also how to request a UDP load balancer from a cloud provider.

## Joining from a native client

From the client's console, using the address in the server's status:

```
\connect <address>
```

Native clients need the server's game files, since they don't download them from
the gateway, so install the same mod and maps for a server that has them.
`server.allowDownload` and `server.downloadURL` set the game's own download cvars
(see the [QuakeServer Reference](quakeserver-reference.md#server)).

## The master server

The controller runs a master server, which lists servers for native clients'
server browsers. Find its address with:

```shell
kubectl --namespace quake-kube get service quake-kube-master
```

and set it from the client's console:

```
seta sv_master1 "<master address>"
```

It lists each server that's ready and has an address, using the address clients
can reach, so servers with `gameServers.exposure.type: None` aren't listed. Like
any master server, it follows what the client asks for: servers with nobody on
them are only listed for clients that ask for empty servers.

`master.service.type` is `LoadBalancer` by default. Use `NodePort` or `ClusterIP`
if native clients don't need it.
