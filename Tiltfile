# -*- mode: Python -*-
#
# Development loop against the local k3d cluster. Run `task dev`, which
# creates the cluster (hack/dev.sh up) and then starts tilt.

CONTEXT = 'k3d-quake-kube'
NAMESPACE = 'quake-kube'

# Traefik's address from hack/dev.sh, resolved by sslip.io.
WEB_HOST = 'quake.172-28-0-100.sslip.io'

if k8s_context() != CONTEXT:
    fail('expected kube context %s, got %s. Run `task cluster` first.' % (CONTEXT, k8s_context()))

# Regenerate the chart's CRD manifests when the resource definitions change.
# The output is committed so the chart works without a Rust toolchain.
local_resource(
    'crds',
    cmd='cd controllers && cargo run --quiet -- crdgen ../chart/files/crds',
    deps=['controllers/src', 'controllers/Cargo.toml', 'controllers/Cargo.lock'],
    labels=['controller'],
)

# Images are built with nix from the working tree, the same way the flake builds
# the published ones (see hack/nix-image.sh).
NIX_DEPS = ['flake.nix', 'flake.lock', 'nix']

custom_build(
    'ghcr.io/chrisrx/quake-kube-controller',
    'hack/nix-image.sh controller-image $EXPECTED_REF',
    deps=NIX_DEPS + ['controllers'],
    ignore=['controllers/target'],
)

# The game server image. The controller receives it through the
# QUAKE_SERVER_IMAGE env var and uses it for the pods it creates.
custom_build(
    'ghcr.io/chrisrx/quake-kube-server',
    'hack/nix-image.sh server-image $EXPECTED_REF',
    deps=NIX_DEPS + ['go.mod', 'go.sum', 'gomod2nix.toml', 'cmd', 'internal'],
    match_in_env_vars=True,
)

k8s_yaml(blob("""
apiVersion: v1
kind: Namespace
metadata:
  name: %s
""" % NAMESPACE))
k8s_yaml(helm(
    'chart',
    name='quake-kube',
    namespace=NAMESPACE,
    # Commas in --set values must be escaped.
    set=[
        'controller.logLevel=info\\,quake_kube_controller=debug',
        'gateway.logLevel=info\\,quake_kube_controller=debug\\,quake_kube_controller::webtransport=trace',
        # The cleanup is a Helm hook for uninstalling, which Tilt runs as a plain
        # Job on every deploy, deleting the servers created from the web app.
        'gateway.deleteServersOnUninstall=false',
        'gateway.ingress.enabled=true',
        'gateway.ingress.host=' + WEB_HOST,
        # Browsers only offer WebTransport to HTTPS pages. Trust the local CA
        # from `hack/dev.sh ca` to avoid certificate warnings.
        'gateway.ingress.annotations.cert-manager\\.io/cluster-issuer=local-ca',
        'gateway.ingress.tls[0].secretName=quake-kube-web-tls',
        'gateway.ingress.tls[0].hosts[0]=' + WEB_HOST,
        # Share Traefik's address (see hack/dev.sh) so the web app's hostname
        # reaches WebTransport too.
        'gateway.webtransport.enabled=true',
        'gateway.webtransport.service.annotations.metallb\\.io/allow-shared-ip=quake-kube-web',
        'gateway.webtransport.service.annotations.metallb\\.io/loadBalancerIPs=172.28.0.100',
    ],
    values=['hack/k3d/presets.yaml'],
))

k8s_resource(
    new_name='quake-kube-crds',
    objects=['quakeservers.quake.kube:customresourcedefinition'],
    resource_deps=['crds'],
    labels=['controller'],
)
k8s_resource(
    'quake-kube-controller',
    objects=[
        '%s:namespace' % NAMESPACE,
        'quake-kube:serviceaccount',
        'quake-kube-controller:clusterrole',
        'quake-kube-controller:clusterrolebinding',
        'quake-kube-controller:configmap',
    ],
    resource_deps=['quake-kube-crds'],
    labels=['controller'],
)

k8s_resource(
    'quake-kube-gateway',
    objects=[
        'quake-kube-gateway:serviceaccount',
        'quake-kube-gateway:clusterrole',
        'quake-kube-gateway:clusterrolebinding',
        'quake-kube-gateway:role',
        'quake-kube-gateway:rolebinding',
        'quake-kube-gateway:ingress',
    ],
    resource_deps=['quake-kube-crds'],
    links=[link('https://' + WEB_HOST, 'web app')],
    labels=['gateway'],
)

# Example server, alongside the servers created from the web app. Its pod is
# owned by the QuakeServer, so Tilt shows its status and logs under this
# resource.
example = decode_yaml(read_file('controllers/example.yaml'))
example['metadata']['namespace'] = NAMESPACE
k8s_yaml(encode_yaml(example))
k8s_resource(
    new_name='example-server',
    objects=['quake-kube:quakeserver:%s' % NAMESPACE],
    resource_deps=['quake-kube-controller'],
    labels=['examples'],
)

# Content images for the presets in hack/k3d/presets.yaml. Tilt can't put its
# own image names in the gateway's config, so they're pushed with fixed tags.
# Building one downloads the mod, 1.4 GB for Urban Terror, which nix keeps.
for name, tag in [('urban-terror', '4.3.4'), ('cpma', '1.53'), ('openarena', '0.8.8'), ('rocket-arena', '1.76'), ('quake3-maps', '1'), ('ufreeze', '1.21'), ('defrag', '1.91.31')]:
    local_resource(
        'content-' + name,
        cmd='hack/nix-image.sh %s-image 127.0.0.1:5005/quake-kube-%s:%s --push' % (name, name, tag),
        deps=NIX_DEPS + ['content/' + name],
        labels=['content'],
    )
