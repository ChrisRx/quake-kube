"""Texture similarity: features that survive a texture being redrawn rather than traced,
like its colors, the direction and density of its edges, and the scale of its patterns."""
import glob
import os

import numpy as np
from PIL import Image

N = 64


def load(path):
    im = Image.open(path)
    w, h = im.size
    rgba = np.asarray(im.convert("RGBA").resize((N, N), Image.BILINEAR), dtype=np.float32) / 255
    return (w, h), rgba


def features(rgba):
    rgb, a = rgba[..., :3], rgba[..., 3]
    hsv = np.asarray(Image.fromarray((rgb * 255).astype(np.uint8)).convert("HSV"), dtype=np.float32) / 255
    hist, _ = np.histogramdd(hsv.reshape(-1, 3), bins=(8, 4, 4), range=((0, 1),) * 3)
    hist = hist.ravel() / hist.sum()
    stats = np.concatenate([rgb.reshape(-1, 3).mean(0), rgb.reshape(-1, 3).std(0)])
    g = rgb @ np.array([0.299, 0.587, 0.114], dtype=np.float32)
    gy, gx = np.gradient(g)
    mag = np.hypot(gx, gy)
    ang = np.mod(np.arctan2(gy, gx), np.pi)
    hog, _ = np.histogram(ang, bins=8, range=(0, np.pi), weights=mag)
    hog = hog / (hog.sum() + 1e-6)
    edges = np.array([mag.mean() * 4])
    f = np.abs(np.fft.fftshift(np.fft.fft2(g - g.mean()))) ** 2
    yy, xx = np.indices(f.shape) - N // 2
    r = np.hypot(xx, yy)
    th = np.mod(np.arctan2(yy, xx), np.pi)
    radial = np.array([f[(r >= lo) & (r < hi)].sum() for lo, hi in zip([1, 2, 4, 8, 16], [2, 4, 8, 16, 32])])
    angular = np.array([f[(th >= lo) & (th < lo + np.pi / 6) & (r >= 2)].sum() for lo in np.arange(0, np.pi, np.pi / 6)])
    radial = radial / (radial.sum() + 1e-6)
    angular = angular / (angular.sum() + 1e-6)
    alpha = np.array([(a < 0.5).mean()])
    return {"hist": hist, "stats": stats, "hog": hog, "edges": edges, "radial": radial, "angular": angular, "alpha": alpha}


def dist(fa, fb):
    d = {
        "color": 1 - np.minimum(fa["hist"], fb["hist"]).sum(),
        "stats": np.abs(fa["stats"] - fb["stats"]).mean() * 4,
        "orient": np.abs(fa["hog"] - fb["hog"]).sum() / 2,
        "edges": float(np.abs(fa["edges"] - fb["edges"])[0]),
        "scale": np.abs(fa["radial"] - fb["radial"]).sum() / 2,
        "angular": np.abs(fa["angular"] - fb["angular"]).sum() / 2,
        "alpha": float(np.abs(fa["alpha"] - fb["alpha"])[0]) * 2,
    }
    d["combined"] = sum(d.values()) / len(d)
    return d


def index(root):
    """Each image under root by its game path, e.g. textures/base_wall/grill: its size,
    features and file. The path starts at a textures or models directory."""
    out = {}
    for path in glob.glob(os.path.join(root, "**", "*"), recursive=True):
        if not path.lower().endswith((".tga", ".jpg")):
            continue
        parts = os.path.relpath(path, root).lower().split(os.sep)
        top = next((i for i, part in enumerate(parts) if part in ("textures", "models")), None)
        if top is None:
            continue
        name = os.path.splitext("/".join(parts[top:]))[0]
        try:
            size, rgba = load(path)
        except Exception:
            continue
        out[name] = (size, features(rgba), path)
    return out
