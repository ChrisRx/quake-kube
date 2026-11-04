#!/usr/bin/env bash
# Retakes the web app's screenshots in docs/src/images from the browser preview (see
# hack/screenshots.py), with Playwright, its browsers and ImageMagick from nix/dev.nix. They're
# large, so they're built here rather than kept in the dev shell. The games' files come from R2,
# so this needs the network.
#
# Usage: hack/screenshots.sh
set -euo pipefail
cd "$(dirname "$0")/.."

# R2 only lets the games' files be read from certain origins, and this is the local one.
readonly HOST=127.0.0.1 PORT=3000

dev() {
  nix build --impure -f nix/dev.nix "$1" --no-link --print-out-paths
}
site=$(dev play-site)
python=$(dev profile-python)
browsers=$(dev profile-browsers)
magick=$(dev screenshots-magick)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/site" "$work/shots"
ln -s "$site" "$work/site/play"
go build -o "$work/play-site" ./hack/play-site

listening() {
  (exec 3<>"/dev/tcp/$HOST/$PORT") 2>/dev/null
}
if listening; then
  echo "screenshots: $HOST:$PORT is in use, perhaps by task docs:serve" >&2
  exit 1
fi

"$work/play-site" --serve "$work/site" --addr "$HOST:$PORT" </dev/null >/dev/null &
server=$!
trap 'kill "$server"; rm -rf "$work"' EXIT
for _ in {1..50}; do
  if listening; then
    break
  fi
  sleep 0.1
done

PLAYWRIGHT_BROWSERS_PATH=${browsers} "${python}/bin/python3" -I hack/screenshots.py \
  "http://$HOST:$PORT/play/" "$work/shots"

# Twice the width they're shown at in the book.
for shot in "$work"/shots/*.png; do
  "${magick}/bin/magick" "$shot" -resize '1500x>' -strip -colors 256 \
    -define png:compression-level=9 "docs/src/images/$(basename "$shot")"
done
