# Getting Started

## Quick start

### With an existing K8s cluster

Deploy the example manifest:

```shell
$ kubectl apply -f https://raw.githubusercontent.com/ChrisRx/quake-kube/master/example.yaml
```

### Without an existing K8s cluster

Start an instance of Kubernetes locally using [kind](https://kind.sigs.k8s.io/)):

```shell
$ kind create cluster
```

Deploy the example manifest:

```shell
$ kubectl apply -f https://raw.githubusercontent.com/ChrisRx/quake-kube/master/example.yaml
```

This can be used to get the kind node IP address:

```shell
kubectl get nodes -o jsonpath='{.items[?(@.metadata.name=="kind-control-plane")].status.addresses[?(@.type=="InternalIP")].address}'
```

Finally, navigate to `http://<kind node ip>:30001` in the browser.

## Development

### Using tilt

[Tilt](https://tilt.dev/) and [ctlptl](https://github.com/tilt-dev/ctlptl) can be used to quickly build and run everything.

First, using ctlptl create a new local cluster with a container registry:

```shell
ctlptl create cluster kind --registry=ctlptl-registry
```

This also includes the Kubernetes objects necessary to configuring a local registry. Next, simply run tilt:

```shell
tilt up
```

This watches for changes in project files and rebuilds as necessary.
