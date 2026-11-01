# Getting Started

QuakeKube is installed with its Helm chart, which deploys:

- the controller, which runs game servers for `QuakeServer` objects and a
  Quake 3 master server that lists them for native clients
- the gateway, which serves the web app, where people create servers and play
  in the browser

## Install

The chart is published to GitHub's container registry and works without any
configuration:

```shell
helm install quake-kube oci://ghcr.io/chrisrx/quake-kube \
  --namespace quake-kube --create-namespace
```

That installs the latest release. Pin one with `--version`, e.g.
`--version 1.2.0`. The chart deploys the images released with it. To install
changes that aren't released yet, use the `chart` directory of a checkout
instead, which deploys the images built from `main`.

Then reach the web app through a port-forward:

```shell
kubectl --namespace quake-kube port-forward service/quake-kube-gateway 8080:80
```

Open `http://localhost:8080`, pick a game, mode and map under **New server**,
and create it. Once it's ready, **Play** joins it in the browser.

The game servers use the files from the Quake 3 demo, which are covered by the
[Quake 3 demo EULA](https://github.com/ChrisRx/quake-kube/blob/main/internal/quake/server/EULA.txt).
Installing QuakeKube means agreeing to it. Browser clients also show it to
players before downloading the files.

## Native clients

Game servers and the master server get LoadBalancer Services, so native
clients can reach them on clusters with load balancers, from a cloud provider
or [MetalLB](https://metallb.io/). Without one, servers still run and play in
the browser. Their Services just wait for an address. `gameServers.exposure` in
the chart's values switches game servers to NodePort or host ports.

Any Quake 3 client can join a server at the address in its status:

```shell
kubectl get quakeservers --all-namespaces
```

The controller's master server lists every server that has an address, so
clients can find them in their server browser instead. Find its address with:

```shell
kubectl --namespace quake-kube get service quake-kube-master
```

and set it as a master server from the client's console:

```
seta sv_master1 "<master address>"
```

## Publish the web app

To make the web app public, put it behind an ingress with a TLS certificate,
e.g. from [cert-manager](https://cert-manager.io/), and turn on WebTransport.
Browsers only use WebTransport from HTTPS pages. It carries game traffic in
unreliable datagrams, like the UDP the game was built for, where the WebSocket
it otherwise falls back to holds everything up behind a lost packet.

```yaml
gateway:
  ingress:
    enabled: true
    className: traefik
    host: quake.example.com
    annotations:
      cert-manager.io/cluster-issuer: letsencrypt
    tls:
      - secretName: quake-kube-web-tls
        hosts: [quake.example.com]
  webtransport:
    enabled: true
```

```shell
helm upgrade quake-kube oci://ghcr.io/chrisrx/quake-kube \
  --namespace quake-kube --values values.yaml
```

WebTransport runs over HTTP/3, so the gateway serves it from its own UDP
LoadBalancer Service, `quake-kube-gateway-webtransport`, rather than through
the ingress. Browsers connect to the web app's hostname on port 443 unless told
otherwise, so either:

- give the Service the ingress controller's IP address, so the same hostname
  reaches both, e.g. with MetalLB:

  ```yaml
  gateway:
    webtransport:
      service:
        annotations:
          metallb.io/allow-shared-ip: <the ingress Service's sharing key>
          metallb.io/loadBalancerIPs: <the ingress Service's IP>
  ```

- or give it an address of its own and tell browsers where it is:

  ```yaml
  gateway:
    webtransport:
      publicAddress: quake-webtransport.example.com:443
  ```

The gateway uses a self-signed certificate for WebTransport, which browsers
accept by its hash, so it doesn't need one from a CA. With more than one
gateway replica, set `gateway.webtransport.tls.secretName` to a certificate
browsers trust, since each replica would otherwise have its own.

The web app has no login of its own. Anyone who can reach it can create up to
`gateway.maxServers` servers, so put authentication in front of it, e.g. in the
ingress, if that's a problem. Setting `gateway.maxServers` to 0 turns creating
servers off.

## Create servers with kubectl

Servers can also be created as `QuakeServer` objects, in any namespace:

```yaml
apiVersion: quake.kube/v1
kind: QuakeServer
metadata:
  name: deathmatch
spec:
  bot:
    minPlayers: 4
  game:
    type: FreeForAll
  maps:
    - name: q3dm17
    - name: q3dm7
```

```shell
kubectl apply -f deathmatch.yaml
kubectl get quakeservers
```

See [Configuration](configuration.md) for the rest of the spec, and
[Add Mods And Maps](add-custom-maps.md) for running mods. Mods' content images
need Kubernetes 1.35, or 1.33 with the `ImageVolume` feature gate.

## Uninstall

```shell
helm uninstall quake-kube --namespace quake-kube
```

The `QuakeServer` CRD is kept by default (`crds.keep`), and with it every
server. Delete it to remove them too:

```shell
kubectl delete crd quakeservers.quake.kube
```
