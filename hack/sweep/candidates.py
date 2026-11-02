"""Builds the picker's data: for each missing texture, every candidate stand-in, ranked by
how much it looks like the retail original, with images of both.

The candidates are the images under <candidates>/<pak name>/, e.g. OpenArena's paks. The
references are the retail originals under <references>, used only to rank the candidates.
Textures without one are listed too, with candidates in name order.

<flat colors> is z-q3a2oa-textures-v4-080.pk3, for offering each texture's placeholder.

Usage: python3 -I candidates.py <missing.json> <references> <candidates> <flat colors> <out dir>
"""
import json
import os
import re
import sys
import zipfile

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import texsim  # noqa: E402

missing = json.load(open(sys.argv[1]))
ref_root, oa_root, out = sys.argv[2], sys.argv[3], sys.argv[5]
flat = {os.path.splitext(n)[0].lower(): n for n in zipfile.ZipFile(sys.argv[4]).namelist() if n.lower().endswith((".tga", ".jpg"))}
os.makedirs(os.path.join(out, "img"), exist_ok=True)
ref = texsim.index(ref_root)
oa = texsim.index(oa_root)
# How many each shortlist holds.
K = 24


def checker(size):
    a = (np.indices((size, size)) // 8).sum(0) % 2
    return Image.fromarray(np.where(a[..., None], 70, 45).repeat(3, 2).astype(np.uint8))


def tiled(im, tile, n=3):
    im = im.convert("RGBA").resize(tile, Image.LANCZOS)
    sheet = Image.new("RGBA", (tile[0] * n, tile[1] * n))
    for y in range(n):
        for x in range(n):
            sheet.paste(im, (x * tile[0], y * tile[1]))
    return sheet


def full(im, limit=512):
    im = im.convert("RGBA")
    w, h = im.size
    scale = min(1, limit / max(w, h))
    return im.resize((max(1, round(w * scale)), max(1, round(h * scale))), Image.LANCZOS) if scale < 1 else im


def tile_size(w, h, base=64):
    return (base, max(1, round(base * h / w))) if w >= h else (max(1, round(base * w / h)), base)


rendered = {}


def render(key, im, w, h):
    if key not in rendered:
        base = "img/" + re.sub(r"[^a-z0-9_]+", "_", key.lower())
        tiled(im, tile_size(w, h)).save(os.path.join(out, base + ".png"), optimize=True)
        full(im).save(os.path.join(out, base + "_full.png"), optimize=True)
        rendered[key] = base + ".png"
    return rendered[key]


def pak_of(path):
    return os.path.relpath(path, oa_root).split(os.sep)[0]


def file_of(path):
    return os.path.relpath(path, os.path.join(oa_root, pak_of(path))).replace(os.sep, "/")


def small(im):
    return np.asarray(im.convert("RGB").resize((16, 16), Image.BOX), dtype=np.float32) / 255


# Each OpenArena texture, and the quarters of larger ones, at 16x16 for layout matching.
layout_pool = []
for c, (s, _, p) in oa.items():
    im = Image.open(p)
    w, h = im.size
    layout_pool.append((c, "", small(im)))
    if w >= 128 and h >= 128:
        for i, (x, y) in enumerate([(0, 0), (w // 2, 0), (0, h // 2), (w // 2, h // 2)]):
            layout_pool.append((c, f"#q{i + 1}", small(im.crop((x, y, x + w // 2, y + h // 2)))))

W = {"color": 3, "stats": 3, "edges": 2, "scale": 1, "orient": 1, "angular": 0.5, "alpha": 1}
candidates = {}
targets = []
def name_order(key):
    # The same directory, or a shared word, e.g. "grate" or "pewter", first.
    words = {w for w in re.split(r"[_\d]+", key.split("/")[-1]) if len(w) >= 3}
    folder = key.split("/")[-2] if "/" in key else ""
    similar = lambda c: c.split("/")[-2] == folder or bool(words & set(re.split(r"[_\d]+", c.split("/")[-1])))
    return sorted(oa, key=lambda c: (not similar(c), c))


def candidate(ckey):
    c, _, q = ckey.partition("#")
    s, _, p = oa[c]
    if ckey not in candidates:
        im = Image.open(p)
        if q:
            i = int(q[1]) - 1
            w, h = im.size
            x, y = [(0, 0), (w // 2, 0), (0, h // 2), (w // 2, h // 2)][i]
            im = im.crop((x, y, x + w // 2, y + h // 2))
        candidates[ckey] = {
            "name": c + (f" (quarter {q[1]})" if q else ""),
            "pak": pak_of(p),
            "file": file_of(p),
            "size": f"{s[0]}x{s[1]}",
            "img": render(ckey, im, *s),
        }


for name, info in missing.items():
    key = name.lower()
    if key not in ref:
        # Nothing to compare with, so candidates come in name order.
        cands = []
        for c in name_order(key):
            candidate(c)
            cands.append({"key": c, "sources": [], "scores": {}})
        targets.append({
            "name": name, "size": "", "alpha": False, "maps": info["maps"], "shaders": info["shaders"],
            "surfaces": sum(info["maps"].values()), "img": None, "tiled": None, "candidates": cands,
            "flat": flat.get(key),
        })
        print(f"{name}: no reference", flush=True)
        continue
    (rw, rh), f, rpath = ref[key]
    aspect = rw / rh
    pool = [c for c in oa if abs(oa[c][0][0] / oa[c][0][1] - aspect) < 0.01]
    d = {c: texsim.dist(f, oa[c][1]) for c in oa}
    weighted = {c: sum(W[k] * d[c][k] for k in W) / sum(W.values()) for c in oa}
    sources = {}

    def add(ckey, source, scores):
        sources.setdefault(ckey, {"sources": [], "scores": scores})
        if source not in sources[ckey]["sources"]:
            sources[ckey]["sources"].append(source)

    def scores(c, **extra):
        return {"combined": round(d[c]["combined"], 3), "weighted": round(weighted[c], 3), **extra}

    for c in sorted(pool, key=lambda c: d[c]["combined"])[:K]:
        add(c, "texture score", scores(c))
    for c in sorted(pool, key=lambda c: weighted[c])[:K]:
        add(c, "color weighted", scores(c))
    # Similar names: the same directory, or a shared word, e.g. "grate" or "pewter".
    words = {w for w in re.split(r"[_\d]+", key.split("/")[-1]) if len(w) >= 3}
    folder = key.split("/")[-2]
    similar = [c for c in pool if c.split("/")[-2] == folder or words & set(re.split(r"[_\d]+", c.split("/")[-1]))]
    for c in sorted(similar, key=lambda c: d[c]["combined"])[:K]:
        add(c, "similar name", scores(c))
    r = small(Image.open(rpath))
    best = {}
    for c, crop, s in layout_pool:
        if c not in d or (crop and abs(aspect - 1) > 0.01):
            continue
        dist = float(np.sqrt(((s - r) ** 2).mean()))
        if c not in best or dist < best[c][0]:
            best[c] = (dist, crop)
    for c, (dist, crop) in sorted(best.items(), key=lambda kv: kv[1][0])[:K]:
        add(c + crop, "layout", scores(c, layout=round(dist, 3)))
    # Everything else too, same shape first, so nothing is hidden by the shortlists.
    shortlisted = set(sources)
    rest = sorted((c for c in pool if c not in shortlisted), key=lambda c: weighted[c])
    other = sorted((c for c in oa if c not in shortlisted and c not in pool), key=lambda c: weighted[c])
    for c in rest:
        sources[c] = {"sources": [], "scores": scores(c)}
    for c in other:
        sources[c] = {"sources": ["other shape"], "scores": scores(c)}

    cands = []
    for ckey, v in sources.items():
        candidate(ckey)
        cands.append({"key": ckey, **v})

    def order(v):
        tags = [s for s in v["sources"] if s != "other shape"]
        group = 0 if tags else (2 if v["sources"] else 1)
        return (group, -len(tags), v["scores"].get("weighted", 1))
    cands.sort(key=order)
    im = Image.open(rpath)
    # Not "ref_", which the tiled render of "ref:<name>" uses.
    single = "img/orig_" + re.sub(r"[^a-z0-9_]+", "_", key) + ".png"
    tw, th = tile_size(rw, rh, 144)
    im.convert("RGBA").resize((tw, th), Image.NEAREST).save(os.path.join(out, single))
    targets.append({
        "name": name,
        "size": f"{rw}x{rh}",
        "alpha": im.mode in ("RGBA", "LA") and bool((np.asarray(im.convert("RGBA"))[..., 3] < 128).any()),
        "maps": info["maps"],
        "shaders": info["shaders"],
        "surfaces": sum(info["maps"].values()),
        "img": single,
        "tiled": render("ref:" + key, im, rw, rh),
        "flat": flat.get(key),
        "candidates": cands,
    })
    print(f"{name}: {len(cands)} candidates", flush=True)

targets.sort(key=lambda t: -t["surfaces"])
with open(os.path.join(out, "data.js"), "w") as fh:
    fh.write("const TARGETS = " + json.dumps(targets) + ";\nconst CANDIDATES = " + json.dumps(candidates) + ";\n")
print(len(targets), "targets,", len(candidates), "candidate images")
