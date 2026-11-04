#!/usr/bin/env bash
# Uploads the games' files to R2 for the browser preview on the docs site (see hack/play-site).
# They don't all fit on GitHub Pages, and it doesn't reliably answer the Range requests they're
# read with. They only change with their versions.
#
# Reads the bucket's credentials from ~/.config/quake-kube/r2.env, which sets R2_ACCOUNT_ID,
# R2_ACCESS_KEY_ID and R2_SECRET_ACCESS_KEY.
set -euo pipefail
cd "$(dirname "$0")/.."

# shellcheck disable=SC1090
source "${R2_ENV:-$HOME/.config/quake-kube/r2.env}"
export AWS_ACCESS_KEY_ID=$R2_ACCESS_KEY_ID
export AWS_SECRET_ACCESS_KEY=$R2_SECRET_ACCESS_KEY
export AWS_DEFAULT_REGION=auto
# R2 doesn't support the checksums newer AWS clients send by default.
export AWS_REQUEST_CHECKSUM_CALCULATION=when_required
export AWS_RESPONSE_CHECKSUM_VALIDATION=when_required

upload() {
  local package=$1 game=$2 content
  content=$(nix build --impure -f nix/dev.nix "$package" --no-link --print-out-paths)
  aws s3 sync "$content/$game" "s3://quake-kube/$game" \
    --endpoint-url "https://${R2_ACCOUNT_ID}.r2.cloudflarestorage.com" \
    --size-only --exclude 'quake-kube-profile*' --no-progress
}

upload openarena-content baseoa
upload cpma-content cpma
upload urban-terror-content q3ut4
