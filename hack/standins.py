"""Finds stand-ins for the retail Quake 3 textures maps need, from other content such as
OpenArena's GPL paks, which has same-named replacements for many of them.

For each map, reads the shaders its BSP uses and resolves them against the base paks: shader
definitions in scripts/*.shader, or images (.tga or .jpg) by name. Whatever doesn't resolve is
looked up in the source paks, and the stand-ins found are written as:

  <out>/images.txt         images to copy, one per line as "<source pak name> <path>"
  <out>/standins.shader    the source's definitions of missing shaders

Usage: python3 -I hack/standins.py <out dir> --base PAK_OR_DIR... --source PAK_OR_DIR...
       --maps PAK_OR_DIR...
"""
import argparse
import os
import re
import struct
import sys
import zipfile


def paks(paths):
    out = []
    for p in paths:
        if os.path.isdir(p):
            out += sorted(os.path.join(p, f) for f in os.listdir(p) if f.lower().endswith(".pk3"))
        else:
            out.append(p)
    return out


def shader_blocks(text):
    """Yields (name, raw text) for each top-level shader definition."""
    text = re.sub(r"//[^\n]*", "", text)
    depth, name, start = 0, None, 0
    for m in re.finditer(r"\{|\}|[^\s{}]+", text):
        t = m.group(0)
        if t == "{":
            if depth == 0:
                start = m.start()
            depth += 1
        elif t == "}":
            depth -= 1
            if depth == 0 and name:
                yield name, f"{name}\n{text[start:m.end()]}\n"
                name = None
        elif depth == 0:
            name = t


class Content:
    """Files and shader definitions across paks, later paks overriding earlier ones."""

    def __init__(self, pak_paths):
        self.files = {}
        self.shaders = {}
        for path in pak_paths:
            z = zipfile.ZipFile(path)
            for n in z.namelist():
                ln = n.lower().replace("\\", "/")
                self.files[ln] = (path, n)
                if ln.startswith("scripts/") and ln.endswith(".shader"):
                    for name, raw in shader_blocks(z.read(n).decode("latin-1")):
                        self.shaders[name.lower()] = raw

    def image(self, name):
        name = re.sub(r"\.(tga|jpg)$", "", name.lower().replace("\\", "/"))
        for ext in (".tga", ".jpg"):
            if name + ext in self.files:
                return name + ext
        return None


def images_of(shader):
    out = []
    toks = shader.split()
    for k, t in enumerate(toks):
        tl = t.lower()
        if tl in ("map", "clampmap") and k + 1 < len(toks) and not toks[k + 1].startswith("$"):
            out.append(toks[k + 1])
        elif tl == "animmap":
            j = k + 2
            while j < len(toks) and "/" in toks[j]:
                out.append(toks[j])
                j += 1
        elif tl == "skyparms" and k + 1 < len(toks) and toks[k + 1] != "-":
            out += [f"{toks[k + 1]}_{s}" for s in ("rt", "lf", "bk", "ft", "up", "dn")]
    return out


def bsp_shaders(data):
    if data[:4] != b"IBSP":
        return set()
    lumps = [struct.unpack("<ii", data[8 + 8 * k : 16 + 8 * k]) for k in range(17)]
    names = set()
    for lump in (1, 12):  # shaders and fogs
        off, length = lumps[lump]
        for k in range(length // 72):
            names.add(data[off + 72 * k : off + 72 * k + 64].split(b"\0")[0].decode("latin-1").lower())
    return names


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("out")
    parser.add_argument("--base", nargs="+", required=True)
    parser.add_argument("--source", nargs="+", required=True)
    parser.add_argument("--maps", nargs="+", required=True)
    args = parser.parse_args()

    map_paks = paks(args.maps)
    base = Content(paks(args.base) + map_paks)
    source = Content(paks(args.source))

    images, shaders, unresolved = set(), {}, {}
    for path in map_paks:
        z = zipfile.ZipFile(path)
        for bsp in (n for n in z.namelist() if n.lower().endswith(".bsp")):
            name = os.path.basename(bsp)[:-4].lower()
            for shader in sorted(bsp_shaders(z.read(bsp))):
                if shader == "noshader" or shader.startswith("textures/common/"):
                    continue
                # Defined, so only its images can be missing.
                if shader in base.shaders:
                    needed = images_of(base.shaders[shader])
                elif base.image(shader):
                    continue
                elif shader in source.shaders:
                    shaders[shader] = source.shaders[shader]
                    needed = images_of(source.shaders[shader])
                else:
                    needed = [shader]
                for image in needed:
                    if base.image(image):
                        continue
                    found = source.image(image)
                    if found:
                        images.add(found)
                    else:
                        unresolved.setdefault(name, set()).add(image)

    os.makedirs(args.out, exist_ok=True)
    with open(os.path.join(args.out, "images.txt"), "w") as f:
        for image in sorted(images):
            pak, name = source.files[image]
            f.write(f"{os.path.basename(pak)} {name}\n")
    with open(os.path.join(args.out, "standins.shader"), "w") as f:
        f.write("// Stand-ins from OpenArena (GPLv2) for retail Quake 3 shaders.\n\n")
        f.writelines(shaders[s] + "\n" for s in sorted(shaders))
    print(f"{len(images)} images and {len(shaders)} shaders found", file=sys.stderr)
    for name, missing in sorted(unresolved.items()):
        print(f"{name}: still missing {len(missing)}: {', '.join(sorted(missing))}", file=sys.stderr)


if __name__ == "__main__":
    main()
