"""Lists the textures maps draw that no pak provides, as JSON: for each image, the shaders
that use it and how many surfaces it's on in each map.

Only surfaces that are drawn count (the BSP's faces), so textures on hidden brush sides
don't. A shader resolves if a script defines it and its stages' images exist, or if an image
has its name.

Usage: python3 -I audit.py PAK... > missing.json
"""
import collections
import json
import os
import re
import struct
import sys
import zipfile


def stage_images(body):
    out = []
    for i, t in enumerate(body):
        if t in ("map", "clampmap") and i + 1 < len(body):
            out.append(body[i + 1])
        elif t == "animmap":
            j = i + 2
            while j < len(body) and "/" in body[j]:
                out.append(body[j])
                j += 1
        elif t == "skyparms" and i + 1 < len(body) and body[i + 1] != "-":
            out += [body[i + 1] + "_" + s for s in ("rt", "bk", "lf", "ft", "up", "dn")]
    return [o for o in out if not o.startswith("$")]


def main(paks):
    images, shaders, bsps = set(), {}, []
    for p in paks:
        z = zipfile.ZipFile(p)
        for n in z.namelist():
            low = n.lower()
            base, ext = os.path.splitext(low)
            if ext in (".tga", ".jpg", ".png"):
                images.add(base)
            elif ext == ".shader":
                text = re.sub(r"//[^\n]*", "", z.read(n).decode("latin1"))
                depth, name, body = 0, None, []
                for tok in re.findall(r'\{|\}|"[^"]*"|[^\s{}]+', text):
                    if tok == "{":
                        depth += 1
                    elif tok == "}":
                        depth -= 1
                        if depth == 0 and name:
                            shaders[name] = body
                            name, body = None, []
                    elif depth == 0:
                        name, body = tok.lower().strip('"'), []
                    else:
                        body.append(tok.lower())
            elif ext == ".bsp" and low.startswith("maps/"):
                bsps.append((os.path.basename(low)[:-4], z.read(n)))

    have = lambda name: os.path.splitext(name.replace("\\", "/"))[0] in images
    out = {}
    for bsp, d in bsps:
        o, size = struct.unpack_from("<ii", d, 16)
        fo, fl = struct.unpack_from("<ii", d, 8 + 8 * 13)
        drawn = collections.Counter(struct.unpack_from("<i", d, fo + j * 104)[0] for j in range(fl // 104))
        for i in range(size // 72):
            if not drawn[i]:
                continue
            shader = d[o + i * 72:o + i * 72 + 64].split(b"\0")[0].decode("latin1").lower().replace("\\", "/")
            if shader == "noshader" or shader.startswith("textures/common/"):
                continue
            missing = [img for img in stage_images(shaders[shader]) if not have(img)] if shader in shaders else ([] if have(shader) else [shader])
            for img in missing:
                e = out.setdefault(os.path.splitext(img.replace("\\", "/"))[0], {"shaders": [], "maps": {}})
                if shader not in e["shaders"]:
                    e["shaders"].append(shader)
                e["maps"][bsp] = e["maps"].get(bsp, 0) + drawn[i]
    json.dump(out, sys.stdout, indent=1, sort_keys=True)


if __name__ == "__main__":
    main(sys.argv[1:])
