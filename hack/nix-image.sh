#!/usr/bin/env bash
# Builds an image from the working tree with nix (see nix/dev.nix), loads it
# and tags it, pushing it too with --push.
#
# Nix builds images reproducibly, so an unchanged image keeps its ID and Tilt
# doesn't replace anything.
#
# Usage: hack/nix-image.sh <package> <ref> [--push]
set -euo pipefail

# podman's docker command uses CONTAINER_HOST rather than DOCKER_HOST, and
# without it loads into a different image store than Tilt uses.
if [[ -n "${DOCKER_HOST:-}" ]]; then
  export CONTAINER_HOST="${CONTAINER_HOST:-${DOCKER_HOST}}"
fi

package=$1 ref=$2
cd "$(dirname "$0")/.."

archive=$(nix build --impure --no-link --print-out-paths -f nix/dev.nix "${package}")
# docker prints "Loaded image: <name>", podman "Loaded image(s): <name>", which
# podman sometimes writes to stderr.
output=$(docker load -i "${archive}" 2>&1)
loaded=$(sed -nE 's/^Loaded image(\(s\))?: //p' <<<"${output}" | tail -1)
if [[ -z "${loaded}" ]]; then
  echo "loading ${archive} didn't name an image:" >&2
  echo "${output}" >&2
  exit 1
fi
docker tag "${loaded}" "${ref}"
if [[ "${3:-}" == "--push" ]]; then
  docker push "${ref}"
fi
