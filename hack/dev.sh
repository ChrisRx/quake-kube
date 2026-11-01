#!/usr/bin/env bash
#
# Manages the local k3d development cluster.
#
#   hack/dev.sh up     Create the cluster and install MetalLB and cert-manager
#   hack/dev.sh down   Delete the cluster and its registry
#   hack/dev.sh tilt   Run tilt against the cluster
#   hack/dev.sh ca     Write the local CA certificate, to trust in a browser
#
# The cluster mirrors a typical cloud setup: Traefik for ingress, real
# LoadBalancer IPs (MetalLB in place of a cloud load balancer) and cert-manager
# with a local CA. Everything here is infrastructure the chart expects to
# already exist, so none of it is installed by the chart itself.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLUSTER=quake-kube
CONTEXT="k3d-${CLUSTER}"
REGISTRY="${CLUSTER}-registry"
REGISTRY_PORT=5005

K3D_SUBNET="${K3D_SUBNET:-172.28.0.0/24}"
METALLB_VERSION="${METALLB_VERSION:-0.16.1}"
CERT_MANAGER_VERSION="${CERT_MANAGER_VERSION:-v1.21.3}"

if [[ "${K3D_SUBNET}" != *.0/24 ]]; then
  echo "K3D_SUBNET must be a /24 ending in .0, got ${K3D_SUBNET}" >&2
  exit 1
fi
NET="${K3D_SUBNET%.0/24}"
# Nodes, the registry and k3d's load balancer get addresses from both ends of
# the subnet, so the pool sits in the middle.
METALLB_POOL="${NET}.100-${NET}.199"
TRAEFIK_IP="${NET}.100"
SHARED_IP_KEY=quake-kube-web

# k3d and Tilt talk to the Docker API. With podman, use the rootful socket:
# rootless podman networks aren't reachable from the host, so neither are
# MetalLB addresses.
if [[ -z "${DOCKER_HOST:-}" && ! -S /var/run/docker.sock && -S /run/podman/podman.sock ]]; then
  export DOCKER_HOST=unix:///run/podman/podman.sock
  export DOCKER_SOCK=/run/podman/podman.sock
fi

# The registry container starts on the runtime's default network, which podman
# names "podman" instead of "bridge".
if [[ "${DOCKER_HOST:-}" == *podman* ]]; then
  REGISTRY_NETWORK="${REGISTRY_NETWORK:-podman}"
  # Podman's Docker API has no BuildKit sessions. Its builder still supports
  # RUN --mount=type=cache.
  export DOCKER_BUILDKIT=0
else
  REGISTRY_NETWORK="${REGISTRY_NETWORK:-bridge}"
fi

kc() { kubectl --context "${CONTEXT}" "$@"; }

# Retries a command while admission webhooks finish starting.
retry() {
  local n
  for n in $(seq 1 30); do
    if "$@" 2>/dev/null; then
      return 0
    fi
    sleep 2
  done
  "$@"
}

# Older k3d releases inject a cgroup v2 entrypoint that relies on "busybox
# xargs", which current k3s images no longer have, so the server never starts.
check_k3d() {
  local version
  version="$(k3d version | sed -n 's/^k3d version v\([0-9]*\.[0-9]*\).*/\1/p')"
  if [[ -z "${version}" ]] || (( ${version%.*} < 5 || (${version%.*} == 5 && ${version#*.} < 8) )); then
    echo "k3d 5.8 or newer is required, found: $(k3d version | head -1)" >&2
    exit 1
  fi
}

up() {
  check_k3d

  if ! k3d registry get "k3d-${REGISTRY}" >/dev/null 2>&1; then
    k3d registry create "${REGISTRY}" \
      --port "127.0.0.1:${REGISTRY_PORT}" \
      --default-network "${REGISTRY_NETWORK}"
  fi

  if k3d cluster get "${CLUSTER}" >/dev/null 2>&1; then
    echo "Cluster ${CLUSTER} already exists"
  else
    k3d cluster create --config "${ROOT}/hack/k3d/cluster.yaml" --subnet "${K3D_SUBNET}"
  fi

  echo "Installing MetalLB ${METALLB_VERSION}"
  helm --kube-context "${CONTEXT}" upgrade --install metallb metallb \
    --repo https://metallb.github.io/metallb \
    --version "${METALLB_VERSION}" \
    --namespace metallb-system --create-namespace \
    --wait
  retry kc apply -f - <<EOF
apiVersion: metallb.io/v1beta1
kind: IPAddressPool
metadata:
  name: default
  namespace: metallb-system
spec:
  addresses:
    - ${METALLB_POOL}
---
apiVersion: metallb.io/v1beta1
kind: L2Advertisement
metadata:
  name: default
  namespace: metallb-system
EOF

  # Pin Traefik to the first pool address so ingress hostnames stay stable
  # across cluster rebuilds. The gateway's WebTransport Service (UDP 443)
  # shares the address, so the web app's hostname works for both.
  kc apply -f - <<EOF
apiVersion: helm.cattle.io/v1
kind: HelmChartConfig
metadata:
  name: traefik
  namespace: kube-system
spec:
  valuesContent: |-
    service:
      annotations:
        metallb.io/loadBalancerIPs: ${TRAEFIK_IP}
        metallb.io/allow-shared-ip: ${SHARED_IP_KEY}
EOF

  echo "Installing cert-manager ${CERT_MANAGER_VERSION}"
  helm --kube-context "${CONTEXT}" upgrade --install cert-manager cert-manager \
    --repo https://charts.jetstack.io \
    --version "${CERT_MANAGER_VERSION}" \
    --namespace cert-manager --create-namespace \
    --set crds.enabled=true \
    --wait
  retry kc apply -f "${ROOT}/hack/k3d/issuers.yaml"

  echo "Waiting for Traefik"
  retry kc -n kube-system rollout status deployment/traefik --timeout=5s

  cat <<EOF

Cluster ${CLUSTER} is ready (context ${CONTEXT}).
  Traefik:       ${TRAEFIK_IP} (e.g. http://quake.${TRAEFIK_IP//./-}.sslip.io)
  MetalLB pool:  ${METALLB_POOL}
  Registry:      localhost:${REGISTRY_PORT}
  Issuers:       local-ca (ClusterIssuer)
EOF
}

down() {
  k3d cluster delete "${CLUSTER}"
  k3d registry delete "k3d-${REGISTRY}"
}

# The local CA signs the web app's certificate. Browsers need to trust it to
# open the web app over HTTPS without warnings, which WebTransport requires.
write_ca() {
  local out="${ROOT}/hack/k3d/local-ca.crt"
  kc -n cert-manager get secret local-ca -o jsonpath='{.data.ca\.crt}' | base64 -d > "${out}"
  echo "Wrote ${out}. Import it into your browser as a trusted certificate authority."
}

tilt_up() {
  exec tilt up --context "${CONTEXT}" "$@"
}

case "${1:-}" in
  up) up ;;
  down) down ;;
  tilt) shift; tilt_up "$@" ;;
  ca) write_ca ;;
  *)
    echo "usage: $0 up|down|tilt|ca" >&2
    exit 1
    ;;
esac
