# Getting Started

QuakeKube is installed with its Helm chart, which deploys:

- the controller, which runs game servers for `QuakeServer` objects and a
  Quake 3 master server that lists them for native clients
- the gateway, which serves the web app, where people create servers and play
  in the browser

## Requirements

- A Kubernetes cluster with x86_64 or aarch64 nodes. Most games use content
  images, which need Kubernetes 1.35, or 1.33 with the `ImageVolume` feature gate.
  Older clusters only offer Quake 3 with the demo's four maps.
- [Helm](https://helm.sh/) 3.8 or newer, for charts in OCI registries.
- A browser with WebAssembly and WebGL, or a native Quake 3 client.

## Install

The chart is published to GitHub's container registry and works without any
configuration:

```shell
helm install quake-kube oci://ghcr.io/chrisrx/quake-kube \
  --namespace quake-kube --create-namespace
```

Then reach the web app through a port-forward:

```shell
kubectl --namespace quake-kube port-forward service/quake-kube-gateway 8080:80
```

Open `http://localhost:8080`, press **Create server**, pick a game, mode and maps,
and create it. Once it's ready, **Play** joins it in the browser. See
[Using The Web App](web-app.md) for the rest, and [Games](games.md) for what's
offered.

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

or find it through the controller's master server. See
[Exposing Game Servers](networking.md) for both.

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

By default, anyone who can reach the web app can create up to
`gateway.maxServers` servers, and change or delete the ones created there. To
sign people in, e.g. with Google Cloud's Identity-Aware Proxy, and give servers
owners, see [Authentication](authentication.md). Setting `gateway.maxServers` to
0 turns creating servers off. See [Security](security.md) for the rest of what to
know before publishing it.

The chart can also route to the web app with Gateway API instead of an
ingress, with `gateway.httpRoute`.

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

## Upgrade

There are no tagged releases. The chart is published from every change to the
main branch, and deploys the newest images. To update an install to them:

```shell
helm upgrade quake-kube oci://ghcr.io/chrisrx/quake-kube \
  --namespace quake-kube --values values.yaml
```

leaving out `--values` if you don't have any. Helm resets anything set with
`--set` on an earlier install unless it's set again.

This restarts the controller and gateway, and updates the CRD. Running game
servers keep their image until their pods are replaced, so they aren't interrupted.
A server gets the new image the next time it's created or switches games.

## Uninstall

```shell
helm uninstall quake-kube --namespace quake-kube
```

Servers created from the web app are deleted with it
(`gateway.deleteServersOnUninstall`). Ones created with kubectl are left
running, along with the `QuakeServer` CRD, which is kept by default
(`crds.keep`). Delete the CRD to remove them too:

```shell
kubectl delete crd quakeservers.quake.kube
```
