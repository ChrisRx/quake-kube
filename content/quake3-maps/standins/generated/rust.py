# Generates textures/base_trim/rusty_pewter_big.tga, a stand-in for Quake 3's texture of
# the same name: OpenArena's dirty_pewter_big (GPL), with a band of rust added along its
# top that stains the metal below it. The rust is procedural, and wraps horizontally so the
# texture still tiles side to side. The committed image is this script's output:
#
#   python3 rust.py dirty_pewter_big.jpg textures/base_trim/rusty_pewter_big.tga
#
# where dirty_pewter_big.jpg is from OpenArena 0.8.8's pak4-textures.pk3. It needs numpy
# and Pillow.
import sys

import numpy as np
from PIL import Image


def noise(shape, scale, rng):
    # Smooth noise that wraps in both directions: white noise low-passed in the frequency
    # domain. scale is the feature size in pixels.
    h, w = shape
    f = np.fft.fft2(rng.standard_normal(shape))
    ky, kx = np.meshgrid(np.fft.fftfreq(h), np.fft.fftfreq(w), indexing="ij")
    f *= np.exp(-((kx**2 + ky**2) * scale**2))
    x = np.real(np.fft.ifft2(f))
    return (x - x.min()) / (x.max() - x.min() + 1e-9)


def rust(src, band=0.07, wobble=0.012, fray_width=0.045, stain=0.05, saturation=0.7, brightness=0.75,
         metal=0.3, seed=3):
    rng = np.random.default_rng(seed)
    img = np.asarray(src.convert("RGB"), dtype=np.float32) / 255
    h, w, _ = img.shape
    y = np.arange(h, dtype=np.float32)[:, None] / h
    # The band's lower edge wanders along the width, and frays into specks: the rust is
    # wherever grain beats the distance below the edge.
    edge_line = noise((1, w), w / 24, rng)[0]
    edge = band + (edge_line - 0.5) * 2 * wobble
    grain = noise((h, w), 2.5, rng)
    fray = noise((h, w), 1.2, rng)
    p = (edge[None, :] - y) / fray_width + (fray - 0.5) * 3.2 + (grain - 0.5) * 1.6
    inside = np.clip(p * 2.5 + 0.5, 0, 1)
    # Rust: patches of black, dark brown, red and orange at a few scales, pitted, with
    # bright orange flecks.
    big = noise((h, w), 12, rng)
    small = noise((h, w), 4, rng)
    pits = noise((h, w), 1.5, rng)
    v = np.clip(big * 0.65 + small * 0.55 + (grain - 0.5) * 0.7 - 0.1, 0, 1)
    stops = [
        (0.00, (0.04, 0.03, 0.02)),  # black
        (0.30, (0.20, 0.08, 0.03)),  # dark brown
        (0.55, (0.45, 0.17, 0.06)),  # red brown
        (0.78, (0.74, 0.36, 0.11)),  # orange
        (1.00, (0.88, 0.55, 0.22)),  # bright orange
    ]
    xs = [p for p, _ in stops]
    color = np.stack([np.interp(v, xs, [c[i] for _, c in stops]) for i in range(3)], -1)
    flecks = np.clip((grain - 0.66) * 6, 0, 1)[..., None]
    color = color * (1 - flecks) + np.array([0.92, 0.56, 0.20]) * flecks
    color = color * (1 - 0.85 * np.clip((0.33 - pits) * 5, 0, 1)[..., None])
    # Muted toward brown, and partly blended with the metal, so it sits on the pewter
    # rather than looking painted over it.
    lum = color.mean(2, keepdims=True)
    color = (lum + (color - lum) * saturation) * brightness
    color = color * (1 - metal) + img * metal
    out = img * (1 - inside[..., None]) + color * inside[..., None]
    # A brown stain fading below the band.
    below = np.clip(y - edge[None, :], 0, None)
    s = np.exp(-below / stain) * (1 - inside) * (0.55 + 0.45 * noise((h, w), 10, rng))
    tint = np.array([0.42, 0.22, 0.10])
    out = out * (1 - 0.45 * s[..., None]) + tint * 0.45 * s[..., None] * out.mean(2, keepdims=True) * 2.2
    return Image.fromarray((np.clip(out, 0, 1) * 255).astype(np.uint8))


if __name__ == "__main__":
    rust(Image.open(sys.argv[1])).save(sys.argv[2])
