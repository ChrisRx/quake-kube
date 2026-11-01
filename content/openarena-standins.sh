#!/bin/sh
# Builds a pk3 of stand-ins from OpenArena (GPLv2) for the retail Quake 3
# textures that maps need, which OpenArena has same-named replacements for.
# hack/standins.py writes the lists of what to take.
#
# Usage: openarena-standins.sh <OpenArena zip> <images list> <shader file or -> <output pk3>
set -eu

zip=$(realpath "$1")
images=$(realpath "$2")
out=$(realpath "$4")
shader=-
[ "$3" = - ] || shader=$(realpath "$3")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"
mkdir files

# Each line is "<OpenArena pak> <path>".
for pak in $(cut -d' ' -f1 "$images" | sort -u); do
  unzip -q -j "$zip" "openarena-0.8.8/baseoa/$pak" -d paks
  grep "^$pak " "$images" | cut -d' ' -f2- | (cd files && xargs unzip -q "../paks/$pak")
done
if [ "$shader" != - ]; then
  mkdir -p files/scripts
  cp "$shader" files/scripts/openarena-standins.shader
fi

cat > files/openarena-standins.txt <<'TXT'
Stand-ins from OpenArena (https://openarena.ws) for textures from the full
Quake 3, which isn't freely distributable, under the same names. OpenArena is
licensed under the GNU GPL v2, and the source for its media is available from
the OpenArena site.
TXT

rm -f "$out"
cd files && zip -q -r -X "$out" .
