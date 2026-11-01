# Authentication

By default, anyone who can reach the web app can create servers and delete the
ones created there. To know who people are, put a proxy that signs them in in
front of the gateway and tell the gateway how to read who they are, with
`gateway.auth.mode`:

- `none`: the default. Servers have no owners.
- `iap`: Google Cloud's [Identity-Aware Proxy](https://cloud.google.com/iap).
  The gateway checks the assertion IAP signs for each request.
- `header`: another proxy that signs people in, such as
  [oauth2-proxy](https://oauth2-proxy.github.io/oauth2-proxy/), and names them
  in a header.

The gateway doesn't sign people in itself, so it works with whatever your
cluster already uses.

## Owners and admins

With authentication on, a server created from the web app belongs to whoever
created it, which is recorded in its `quake.kube/owner` annotation. Only they
can delete it from the web app, along with admins.

Admins are whoever Kubernetes RBAC lets delete QuakeServers in the web app's
namespace, which the gateway asks the API server about. So anyone who could
already delete servers with `kubectl` can delete them from the web app too, and
admins can be granted in the same way as other access to the cluster. The chart
has a `quake-kube-admin` ClusterRole for this, and binds it to the users and
groups in `gateway.auth.admins`:

```yaml
gateway:
  auth:
    admins:
      users: [ranger@example.com]
      groups: [quake-admins]
```

Servers created before authentication was turned on have no owner, so only
admins can delete them.

`gateway.maxServersPerUser` limits how many servers each person can have at
once, alongside `gateway.maxServers` for everyone together. Admins have the same
limit.

## Google Cloud's Identity-Aware Proxy

On GKE, IAP signs people in with their Google accounts before they reach the
web app. Who can reach it is managed in IAM, by granting the **IAP-secured Web
App User** role, e.g. to a Google group.

IAP works with GKE's load balancers, through Gateway API or a GKE Ingress. The
chart creates GKE's settings for the gateway's Service with `gateway.gke`: a
GCPBackendPolicy for an HTTPRoute, and a BackendConfig for an Ingress. With
Gateway API:

```yaml
gateway:
  auth:
    mode: iap
    iap:
      # The project's number, not its ID.
      projectNumber: "123456789012"
  httpRoute:
    enabled: true
    parentRefs:
      - name: external
        namespace: infra
    hostnames: [quake.example.com]
  gke:
    enabled: true
    iap:
      enabled: true
```

or with a GKE Ingress, replace `httpRoute` with:

```yaml
gateway:
  ingress:
    enabled: true
    className: gce
    host: quake.example.com
```

IAP signs its assertions for the load balancer's backend service, which GKE
creates and names, so the gateway accepts assertions for any backend service in
the project. Set `gateway.auth.iap.audience` to the backend service's audience
instead, `/projects/<number>/global/backendServices/<id>`, to only accept its
own.

Without an OAuth client, IAP uses a Google-managed one, which only lets in
people in your organization. To use your own, set `gateway.gke.iap.clientID`
and `gateway.gke.iap.clientSecretName`.

`gateway.gke` also keeps GKE's load balancers from closing browser clients'
WebSockets after 30 seconds, with `timeoutSec`, so turn it on with GKE's load
balancers even without IAP.

The gateway's own Service isn't behind IAP, so the gateway turns away requests
without a valid assertion, e.g. from inside the cluster.

## Other proxies

With `header`, the gateway reads the user's email address from a header, and
their groups from another, if the proxy sends them. With oauth2-proxy's
defaults:

```yaml
gateway:
  auth:
    mode: header
    header:
      user: X-Forwarded-Email
      groups: X-Forwarded-Groups
```

The gateway trusts these headers, so it must only be reachable through the
proxy, e.g. with a NetworkPolicy that only lets the proxy reach it. Groups from
the proxy are what RBAC checks admins' groups against.

## WebTransport

WebTransport sessions reach the gateway over UDP, past the proxy, so the play
page gives browsers a ticket for the server they're joining, which the gateway
checks. A ticket works for an hour, and browsers that reconnect later use the
WebSocket, which goes through the proxy.

Each gateway replica signs tickets with its own key unless
`gateway.auth.ticketSecretName` names a Secret with a shared `key`, so set it
with more than one replica:

```shell
kubectl --namespace quake-kube create secret generic quake-kube-tickets \
  --from-literal=key="$(head -c 32 /dev/urandom | base64)"
```

## Native clients

Native Quake 3 clients connect to game servers directly, past the proxy. To keep
people from joining, give servers a password (see
[Setting A Password](setting-a-password.md)), or turn off their external
addresses with `gameServers.exposure.type: None`, which leaves them to browser
clients.
