#!/usr/bin/env bash
#
# Manages the local k3d development cluster.
#
#   hack/dev.sh up     Create the cluster and install MetalLB and cert-manager
#   hack/dev.sh down   Delete the cluster and its registry
#   hack/dev.sh tilt   Run tilt against the cluster
#   hack/dev.sh trust  Create a local CA with mkcert, trusted by your browsers,
#                      which every dev cluster then uses
#   hack/dev.sh ca     Write the local CA certificate, to trust in a browser
#   hack/dev.sh ui     Open the web app, with mkcert's CA if there is one
#   hack/dev.sh forward [namespace] [release] [port]
#                      Open the web app through a port-forward, with the
#                      current kubectl context, e.g. for a cluster installed
#                      with the chart
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
WEB_HOST="quake.${TRAEFIK_IP//./-}.sslip.io"
CA_FILE="${ROOT}/hack/k3d/local-ca.crt"
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
  if have_mkcert_ca; then
    use_mkcert_ca
  else
    retry kc apply -f "${ROOT}/hack/k3d/local-ca.yaml"
  fi

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
# With mkcert, it's one CA for the machine, which mkcert has browsers trust and
# every cluster uses. Without it, each cluster makes its own, which needs
# importing into browsers by hand.
current_ca() {
  kc -n cert-manager get secret local-ca -o jsonpath='{.data.tls\.crt}' 2>/dev/null | base64 -d
}

mkcert_caroot() {
  command -v mkcert >/dev/null && mkcert -CAROOT
}

have_mkcert_ca() {
  local caroot
  caroot="$(mkcert_caroot)" && [[ -f "${caroot}/rootCA.pem" && -f "${caroot}/rootCA-key.pem" ]]
}

# Makes mkcert's CA the cluster's, and issues certificates again that the old
# one signed.
use_mkcert_ca() {
  local caroot
  caroot="$(mkcert_caroot)"
  if [[ "$(current_ca)" == "$(cat "${caroot}/rootCA.pem")" ]]; then
    return
  fi
  echo "Using mkcert's CA (${caroot}) for the cluster's certificates"
  kc -n cert-manager delete certificate local-ca --ignore-not-found >/dev/null
  kc -n cert-manager delete secret local-ca --ignore-not-found >/dev/null
  kc -n cert-manager create secret tls local-ca \
    --cert="${caroot}/rootCA.pem" --key="${caroot}/rootCA-key.pem" >/dev/null
  kc get certificates --all-namespaces \
    -o jsonpath='{range .items[?(@.spec.issuerRef.name=="local-ca")]}{.metadata.namespace} {.spec.secretName}{"\n"}{end}' |
    while read -r namespace secret; do
      kc -n "${namespace}" delete secret "${secret}" --ignore-not-found >/dev/null
    done
}

# Creates mkcert's CA if needed and has the browsers trust it. On NixOS the
# system's trust store is declared in its configuration, so only the browsers'
# are changed.
trust() {
  TRUST_STORES=nss mkcert -install
  cat <<EOF
Restart your browsers for them to pick it up. The dev cluster uses this CA from
now on, starting the next time you run task ui or create a cluster.
EOF
}

trust_hint() {
  cat <<EOF
Run task ui:trust once to have your browsers trust a local CA that every dev
cluster uses. Or import ${CA_FILE}, this cluster's own CA, into your browser as
a trusted certificate authority:
  Chrome:  Settings > Privacy and security > Security > Manage certificates >
           Authorities > Import, then trust it for identifying websites.
  Firefox: Settings > Privacy & Security > Certificates > View Certificates >
           Authorities > Import, then trust it to identify websites.
A new cluster has a new CA, which needs importing again.
EOF
}

write_ca() {
  current_ca > "${CA_FILE}"
  echo "Wrote ${CA_FILE}."
  trust_hint
}

open_url() {
  if command -v xdg-open >/dev/null; then
    xdg-open "$1" >/dev/null 2>&1 &
  elif command -v open >/dev/null; then
    open "$1"
  else
    echo "Open $1"
  fi
}

ui() {
  if have_mkcert_ca; then
    use_mkcert_ca
  else
    local ca
    ca="$(current_ca)"
    # Written again when the cluster has been recreated with a new CA.
    if [[ ! -f "${CA_FILE}" || "$(cat "${CA_FILE}")" != "${ca}" ]]; then
      printf '%s\n' "${ca}" > "${CA_FILE}"
      echo "Wrote the cluster's CA certificate to ${CA_FILE}."
      trust_hint
    fi
  fi
  echo "Opening https://${WEB_HOST}"
  open_url "https://${WEB_HOST}"
}

# Uses the current kubectl context rather than the dev cluster's, so it works
# with any cluster the chart is installed in.
forward() {
  local namespace="${1:-quake-kube}" release="${2:-quake-kube}" port="${3:-8080}"
  kubectl --namespace "${namespace}" port-forward "service/${release}-gateway" "${port}:80" &
  local pid=$!
  trap 'kill ${pid} 2>/dev/null' EXIT INT TERM
  local n
  for n in $(seq 1 50); do
    if curl -fsS -o /dev/null "http://localhost:${port}/healthz" 2>/dev/null; then
      echo "Opening http://localhost:${port} (Ctrl-C stops the port-forward)"
      open_url "http://localhost:${port}"
      break
    fi
    sleep 0.2
  done
  wait "${pid}"
}

tilt_up() {
  exec tilt up --context "${CONTEXT}" "$@"
}

case "${1:-}" in
  up) up ;;
  down) down ;;
  tilt) shift; tilt_up "$@" ;;
  ca) write_ca ;;
  trust) trust ;;
  ui) ui ;;
  forward) shift; forward "$@" ;;
  *)
    echo "usage: $0 up|down|tilt|trust|ca|ui|forward" >&2
    exit 1
    ;;
esac
