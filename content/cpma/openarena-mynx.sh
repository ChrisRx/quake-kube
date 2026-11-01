#!/bin/sh
# Builds a pk3 with a stand-in for CPMA's default player model, mynx, from
# OpenArena's "major" model (GPLv2). CPMA's client won't start without mynx,
# whose mesh is only in the full game's pak0.pk3. CPMA ships mynx's skins and
# textures, which this overrides, so the pk3's name has to sort after CPMA's.
#
# Usage: openarena-mynx.sh <OpenArena's pak2-players.pk3> <output pk3>
set -eu

players=$(realpath "$1")
out=$(realpath "$2")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"

unzip -q "$players" 'models/players/major/*' 'sound/player/major/*'

# Everything moves to mynx's paths, since the Quake 3 demo has a major of its
# own that these would replace.
mkdir -p models/players/mynx sound/player/mynx scripts
for f in animation.cfg head.md3 upper.md3 lower.md3 \
  head.tga torso.tga torsored.tga torsoblue.tga lower.tga lowerred.tga lowerblue.tga \
  pm_head.tga pm_torso.tga pm_lower.tga; do
  mv "models/players/major/$f" models/players/mynx/
done
cp models/players/major/icon_default.tga models/players/mynx/icon_default.tga
cp models/players/major/icon_red.tga models/players/mynx/icon_red.tga
cp models/players/major/icon_blue.tga models/players/mynx/icon_blue.tga
# CPMA's own icons for its skins.
cp models/players/major/icon_default.tga models/players/mynx/icon_pm.tga
cp models/players/major/icon_default.tga models/players/mynx/icon_fb.tga
mv sound/player/major/*.wav sound/player/mynx/
rm -r models/players/major sound/player/major

skin() {
  printf 'h_head,%s\ntag_head,\n' "$2" > "models/players/mynx/head_$1.skin"
  printf 'u_torso,%s\ntag_head,\ntag_torso,\ntag_weapon,\n' "$3" > "models/players/mynx/upper_$1.skin"
  printf 'l_lower,%s\ntag_torso,\n' "$4" > "models/players/mynx/lower_$1.skin"
}
t=models/players/mynx
skin default "$t/head.tga" "$t/torso.tga" "$t/lower.tga"
skin red "$t/head.tga" "$t/torsored.tga" "$t/lowerred.tga"
skin blue "$t/head.tga" "$t/torsoblue.tga" "$t/lowerblue.tga"
# CPMA's bright skins: pm is tinted with the player's colors, fb is a flat
# color. fullBright is CPMA's shader.
skin pm "$t/oa_pm_head" "$t/oa_pm_torso" "$t/oa_pm_lower"
skin fb fullBright fullBright fullBright

for part in head torso lower; do
  cat >> scripts/mynx-openarena.shader <<SHADER
$t/oa_pm_$part
{
	nopicmip
	{
		map $t/pm_$part.tga
		rgbGen entity
	}
}

SHADER
done

cat > openarena-mynx.txt <<'TXT'
CPMA's default player model, mynx, isn't freely distributable, so this pk3
stands in for it with OpenArena's "major" model and voice, moved to mynx's
paths. OpenArena (https://openarena.ws) is licensed under the GNU GPL v2, and
the source for its media is available from the OpenArena site.
TXT

rm -f "$out"
zip -q -r -X "$out" models sound scripts openarena-mynx.txt
