#!/usr/bin/env bash
# Builds and serves the stand-in picker for the community maps' missing textures (see
# content/quake3-maps/standins/README.md). It needs the retail Quake 3 baseq3/pak0.pk3,
# whose originals rank the candidates. That file stays where it is, and only what's
# extracted from it goes in .cache/sweep, which git ignores.
#
# Usage: hack/sweep/sweep.sh <retail pak0.pk3> [port]
set -euo pipefail

pak0=$(realpath "${1:?usage: hack/sweep/sweep.sh <retail pak0.pk3> [port]}")
port=${2:-8765}
root=$(git rev-parse --show-toplevel)
here=$root/hack/sweep
cache=$root/.cache/sweep
standins=$root/content/quake3-maps/standins
mkdir -p "$cache"

# The content and what its stand-ins come from, built outside the flake so only the content
# directory is copied to the store, not the working tree. The map downloads come from Cachix.
nixpkgs="let lock = builtins.fromJSON (builtins.readFile $root/flake.lock); in import (builtins.fetchTree lock.nodes.nixpkgs.locked) {}"
content_expr="($nixpkgs).callPackage $root/content/quake3-maps {}"
build() {
  nix build --impure --no-link --print-out-paths \
    --option extra-substituters https://quake-kube.cachix.org \
    --option extra-trusted-public-keys quake-kube.cachix.org-1:0+WG60OtwDRX5Xg39e+qWxtGgqnvkl/SE0sStdg+2wM= \
    --expr "$1"
}
echo "building the content"
python=$(build "($nixpkgs).python3.withPackages (p: [ p.pillow p.numpy ])")/bin/python3
content=$(build "$content_expr")
openarena=$(build "($content_expr).sources.openarena")
udi=$standins/sources/z_missing-textures1_v1.pk3
flat=$standins/sources/z-q3a2oa-textures-v4-080.pk3

# The demo's pak0, from the installer q3 embeds.
if [ ! -f "$cache/demo/pak0.pk3" ]; then
  echo "extracting the demo's pak0"
  mkdir -p "$cache/demo"
  "$python" -I - "$root/internal/quake/server/gamefiles.tar" "$cache/demo/pak0.pk3" <<'EOF'
import gzip, io, sys, tarfile
installer = next(m for m in tarfile.open(sys.argv[1]) if m.name.startswith("linuxq3ademo"))
data = tarfile.open(sys.argv[1]).extractfile(installer).read()
# A shell script followed by a gzipped tar.
i = data.index(b"\x1f\x8b\x08")
inner = tarfile.open(fileobj=io.BytesIO(gzip.decompress(data[i:])))
open(sys.argv[2], "wb").write(inner.extractfile("demoq3/pak0.pk3").read())
EOF
fi

# What maps draw that nothing provides, without the stand-ins, so decided textures are listed
# too and can be revisited. Textures OpenArena has under the same name (standins/images.txt)
# aren't judgment calls, so they're left out.
echo "auditing the maps"
paks=()
for p in "$content"/baseq3/*.pk3; do
  [ "$(basename "$p")" = zz-quake3-maps-openarena-standins.pk3 ] || paks+=("$p")
done
"$python" -I "$here/audit.py" "$cache/demo/pak0.pk3" "${paks[@]}" >"$cache/all-missing.json"
"$python" -I - "$cache/all-missing.json" "$standins/images.txt" "$cache/missing.json" <<'EOF'
import json, os, sys
missing = json.load(open(sys.argv[1]))
same = {os.path.splitext(line.split()[1])[0].lower() for line in open(sys.argv[2]) if line.strip() and not line.startswith("#")}
json.dump({k: v for k, v in missing.items() if k.lower() not in same}, open(sys.argv[3], "w"), indent=1, sort_keys=True)
print(f"{len(json.load(open(sys.argv[3])))} textures to decide")
EOF

# The retail originals of those, to rank candidates against.
echo "extracting references"
rm -rf "$cache/references"
"$python" -I - "$pak0" "$cache/missing.json" "$cache/references" <<'EOF'
import json, os, sys, zipfile
z = zipfile.ZipFile(sys.argv[1])
names = {os.path.splitext(n)[0].lower(): n for n in z.namelist() if n.lower().endswith((".tga", ".jpg"))}
for k in json.load(open(sys.argv[2])):
    n = names.get(k.lower())
    if n:
        dest = os.path.join(sys.argv[3], n.lower())
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        open(dest, "wb").write(z.read(n))
EOF

# The candidates: OpenArena's textures and Udi's, one directory per pak.
if [ ! -d "$cache/candidates" ]; then
  echo "extracting candidates"
  mkdir -p "$cache/candidates.tmp"
  "$python" -I - "$openarena" "$udi" "$cache/candidates.tmp" <<'EOF'
import io, os, sys, zipfile
def extract(z, dest):
    for n in z.namelist():
        low = n.lower()
        if low.endswith((".tga", ".jpg")) and low.startswith(("textures/", "models/mapobjects/")):
            path = os.path.join(dest, low)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            open(path, "wb").write(z.read(n))
oa = zipfile.ZipFile(sys.argv[1])
for n in oa.namelist():
    pak = os.path.basename(n)
    if n.startswith("openarena-0.8.8/baseoa/") and pak.endswith(".pk3") and not pak.startswith("pak2-players"):
        extract(zipfile.ZipFile(io.BytesIO(oa.read(n))), os.path.join(sys.argv[3], pak[:-4]))
extract(zipfile.ZipFile(sys.argv[2]), os.path.join(sys.argv[3], "z_missing-textures1_v1"))
EOF
  mv "$cache/candidates.tmp" "$cache/candidates"
fi

# The page, made again only when what it's made from changes, since it takes a while.
stamp=$(cat "$cache/missing.json" "$here/candidates.py" "$here/texsim.py" | sha256sum | cut -d' ' -f1)
if [ "$(cat "$cache/page/.stamp" 2>/dev/null)" != "$stamp" ]; then
  echo "making the page, which takes several minutes"
  rm -rf "$cache/page"
  (cd "$here" && "$python" -I -B candidates.py "$cache/missing.json" "$cache/references" "$cache/candidates" "$flat" "$cache/page")
  echo "$stamp" >"$cache/page/.stamp"
fi
cp "$here/index.html" "$cache/page/index.html"

echo "decisions are recorded in content/quake3-maps/standins/decisions.jsonl"
exec "$python" -I "$here/server.py" "$cache/page" "$standins/decisions.jsonl" "$port"
