#!/usr/bin/env bash
# Writes a content image's profile from the dev cluster (see hack/profile.py), with
# Playwright and its browsers from nix/dev.nix. They're large, so they're built here
# rather than kept in the dev shell.
#
# Usage: hack/profile.sh <preset> <output>
set -euo pipefail
cd "$(dirname "$0")/.."

python=$(nix build --impure -f nix/dev.nix profile-python --no-link --print-out-paths)
browsers=$(nix build --impure -f nix/dev.nix profile-browsers --no-link --print-out-paths)
PLAYWRIGHT_BROWSERS_PATH=${browsers} exec "${python}/bin/python3" -I hack/profile.py "$@"
