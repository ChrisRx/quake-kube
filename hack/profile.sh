#!/usr/bin/env bash
# Writes a content image's profile from the dev cluster (see hack/profile.py), with
# Playwright and its browsers from the flake's nixpkgs. They're large, so they're
# built here rather than kept in the dev shell.
#
# Usage: hack/profile.sh <preset> <output>
set -euo pipefail
cd "$(dirname "$0")/.."

pkgs='let flake = builtins.getFlake (toString ./.); in import flake.inputs.nixpkgs { system = builtins.currentSystem; }'
python=$(nix build --impure --no-link --print-out-paths \
  --expr "(${pkgs}).python3.withPackages (ps: [ ps.playwright ps.pyyaml ])")
browsers=$(nix build --impure --no-link --print-out-paths \
  --expr "(${pkgs}).playwright-driver.browsers")
PLAYWRIGHT_BROWSERS_PATH=${browsers} exec "${python}/bin/python3" -I hack/profile.py "$@"
