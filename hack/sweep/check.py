"""Checks a stand-in isn't derived from the retail original it replaces, by correlating their
fine detail: grain, scratches and marks, with the broad shapes taken out. Two textures drawn
independently score near 0, even with the same layout. Copies, even blurred, resized or
recolored ones, keep their fine detail and score high; traced work also scores high.

For reference, textures from a pack found to be made from the originals scored 0.5 to 0.99,
and OpenArena's own recreations score below 0.3, apart from simple geometric ones like light
strips, whose edges line up by design.

Usage: python3 -I check.py <stand-in image> <retail original>
"""
import sys

import numpy as np
from PIL import Image, ImageFilter


def gray(im):
    return np.asarray(im.convert("L"), dtype=np.float32) / 255


def fine(im):
    return gray(im) - gray(im.filter(ImageFilter.GaussianBlur(2)))


def corr(a, b):
    a, b = a.ravel() - a.mean(), b.ravel() - b.mean()
    d = np.sqrt((a * a).sum() * (b * b).sum())
    return float((a * b).sum() / d) if d > 1e-6 else 0.0


if __name__ == "__main__":
    standin = Image.open(sys.argv[1]).convert("RGB")
    original = Image.open(sys.argv[2]).convert("RGB").resize(standin.size, Image.LANCZOS)
    if gray(standin).std() < 0.02:
        print("flat: nothing to compare")
    else:
        print(f"fine detail correlation: {corr(fine(standin), fine(original)):.3f}")
