# Generates textures/liquids/bubbles.tga, a stand-in for Quake 3's texture of the same
# name: glowing orbs at different depths, with soft shadows, tendrils between them and
# swirls in the gaps. It's made from scratch, never reading the original, and tiles
# seamlessly since everything is drawn on a torus. chiropterata multiplies it over its
# lava. The committed image is this script's output:
#
#   python3 orbs.py textures/liquids/bubbles.tga
#
# It needs numpy and Pillow.
import sys

import numpy as np
from PIL import Image

def tileable_noise(n, scale, rng):
    # Smooth noise that tiles: white noise low-passed in the frequency domain.
    f = np.fft.fft2(rng.standard_normal((n, n)))
    ky, kx = np.meshgrid(np.fft.fftfreq(n), np.fft.fftfreq(n), indexing="ij")
    f *= np.exp(-((kx**2 + ky**2) * (n / scale) ** 2))
    x = np.real(np.fft.ifft2(f))
    return (x - x.min()) / (x.max() - x.min() + 1e-9)


def poisson_torus(n, count, min_dist, rng, tries=4000):
    pts = []
    for _ in range(tries):
        if len(pts) >= count:
            break
        p = rng.uniform(0, n, 2)
        ok = True
        for q in pts:
            d = np.abs(p - q[:2])
            d = np.minimum(d, n - d)
            if np.hypot(*d) < min_dist:
                ok = False
                break
        if ok:
            pts.append(p)
    return np.array(pts)


def warp_filaments(n, rng, scale=6, width=0.035, warp=0.9):
    # Thin bright lines: the zero crossings of noise, warped by more noise. All tileable.
    base = tileable_noise(n, scale, rng) - 0.5
    wx = (tileable_noise(n, 4, rng) - 0.5) * warp * n / 6
    wy = (tileable_noise(n, 4, rng) - 0.5) * warp * n / 6
    yy, xx = np.mgrid[0:n, 0:n]
    sx = ((xx + wx).round().astype(int)) % n
    sy = ((yy + wy).round().astype(int)) % n
    warped = base[sy, sx]
    return np.exp(-(warped / width) ** 2)


def splat_path(img, pts, widths, values, ss):
    # Draws a tapering bright strand along points (in texture pixels) onto img (supersampled),
    # wrapping at the edges, with the brightest contribution kept per pixel.
    N = img.shape[0]
    for (x, y), w, v in zip(pts, widths, values):
        cx, cy = x * ss, y * ss
        sig = max(w * ss / 2, 0.6)
        R = int(np.ceil(sig * 3))
        ix = np.arange(int(cx) - R, int(cx) + R + 1)
        iy = np.arange(int(cy) - R, int(cy) + R + 1)
        g = np.exp(-(((ix[None, :] - cx) ** 2 + (iy[:, None] - cy) ** 2) / (2 * sig * sig))) * v
        sub = np.ix_(iy % N, ix % N)
        img[sub] = np.maximum(img[sub], g)


def strand(rng, start, heading, length, w0, v0, curl=0.25, step=0.5):
    pts, widths, values = [], [], []
    x, y = start
    n = max(2, int(length / step))
    turn = rng.normal(0, curl)
    for i in range(n):
        f = i / n
        pts.append((x, y)); widths.append(w0 * (1 - 0.8 * f)); values.append(v0 * (1 - f) ** 1.3)
        turn += rng.normal(0, curl * 0.35)
        heading += turn * step / 6
        x += np.cos(heading) * step
        y += np.sin(heading) * step
    return pts, widths, values


def orbs_soft(n=128, ss=4, count=55, dmin=12, dmax=38, bg=0.16, filament=0.18, falloff=1.1, rim=0.5,
              shadow=0.55, shadow_offset=(0.12, 0.18), shadow_width=0.4, glow=0.35, edge=0.22,
              tendrils=(1, 3), tendril_len=(0.6, 1.6), tendril_width=1.1, tendril_value=0.75,
              bridges=0.35, seed=1, min_dist=9.5, fill_gaps=0.0, filler_r=(3.5, 9)):
    rng = np.random.default_rng(seed)
    N = n * ss
    pts = poisson_torus(n, count, min_dist, rng, tries=40000)
    yy, xx = np.mgrid[0:N, 0:N].astype(float) / ss
    orbs = []
    for cx, cy in pts:
        z = rng.uniform(0, 1)
        d = dmin + (dmax - dmin) * rng.uniform(0, 1) ** 1.6
        orbs.append([z, cx, cy, d / 2, False])
    # Fill the widest gaps with small orbs behind the others, from their own random
    # sequence, so the main orbs and their tendrils stay as they were.
    fillers = []
    if fill_gaps:
        frng = np.random.default_rng(seed + 1000)
        gy, gx = np.mgrid[0:n, 0:n].astype(float)
        gap = np.full((n, n), np.inf)
        def carve(cx, cy, r):
            dx = (gx - cx + n / 2) % n - n / 2
            dy = (gy - cy + n / 2) % n - n / 2
            np.minimum(gap, np.sqrt(dx * dx + dy * dy) - r, out=gap)
        for _, cx, cy, r, _ in orbs:
            carve(cx, cy, r)
        for _ in range(200):
            i = int(np.argmax(gap))
            g = gap.flat[i]
            if g < fill_gaps:
                break
            fy, fx = divmod(i, n)
            r = float(np.clip(g * 1.1, *filler_r))
            fillers.append([frng.uniform(0, 0.4), float(fx), float(fy), r, True])
            carve(fx, fy, r)
    main = sorted(orbs)
    orbs = sorted(main + fillers)
    back = bg + 0.06 * (tileable_noise(n, 12, rng) - 0.5)
    if filament:
        back = back + filament * warp_filaments(n, rng)
    img = np.kron(back, np.ones((ss, ss)))
    # Bridges between near neighbors, drawn with the farther orb of each pair.
    bridge_of = {i: [] for i in range(len(main))}
    for i, (zi, xi, yi, ri, _) in enumerate(main):
        for j in range(i + 1, len(main)):
            zj, xj, yj, rj, _ = main[j]
            dx = (xj - xi + n / 2) % n - n / 2
            dy = (yj - yi + n / 2) % n - n / 2
            dist = np.hypot(dx, dy)
            if ri + rj < dist < (ri + rj) * 1.9 and rng.uniform() < bridges:
                bridge_of[i].append((dx, dy, dist, rj))
    frng2 = np.random.default_rng(seed + 2000)
    main_index = {id(o): i for i, o in enumerate(main)}
    for orb in orbs:
        z, cx, cy, r, filler = orb
        k = main_index.get(id(orb))
        orng = frng2 if filler else rng
        depth = 0.55 + 0.45 * z
        # Shadow on everything farther, just outside the rim, away from the light.
        sx, sy = cx + shadow_offset[0] * r, cy + shadow_offset[1] * r
        dx = (xx - sx + n / 2) % n - n / 2
        dy = (yy - sy + n / 2) % n - n / 2
        t = np.sqrt(dx * dx + dy * dy) / r
        # A filled soft disk, mostly hidden under the orb, showing on the side away from the light.
        img *= 1 - shadow * np.exp(-np.clip(t - 0.75, 0, None) ** 2 / shadow_width**2)
        # Tendrils and bridges, at this orb's depth, behind it.
        strands = np.zeros_like(img)
        for _ in range(orng.integers(tendrils[0], tendrils[1] + 1)):
            a = orng.uniform(0, 2 * np.pi)
            start = (cx + np.cos(a) * r * 0.9, cy + np.sin(a) * r * 0.9)
            splat_path(strands, *strand(orng, start, a + orng.normal(0, 0.4), r * orng.uniform(*tendril_len),
                                        tendril_width, tendril_value * depth), ss)
        for bx, by, dist, rj in (bridge_of[k] if k is not None else []):
            a = np.arctan2(by, bx)
            start = (cx + np.cos(a) * r * 0.9, cy + np.sin(a) * r * 0.9)
            splat_path(strands, *strand(rng, start, a + rng.normal(0, 0.25), dist - r * 0.9 - rj * 0.6,
                                        tendril_width * 0.9, tendril_value * depth, curl=0.12), ss)
        img = 1 - (1 - img) * (1 - strands)
        # The orb: a soft glowing dome with a glow spilling past its rim.
        dx = (xx - cx + n / 2) % n - n / 2
        dy = (yy - cy + n / 2) % n - n / 2
        t = np.sqrt(dx * dx + dy * dy) / r
        halo = glow * depth * np.exp(-np.clip(t - 1, 0, None) ** 2 / 0.12) * (t >= 1)
        img = 1 - (1 - img) * (1 - halo)
        body = (rim + (1 - rim) * np.clip(1 - t * t, 0, 1) ** falloff) * depth
        a = np.clip((1 - t) / edge, 0, 1)  # soft edge over the outer part of the radius
        img = img * (1 - a) + body * a
    img = img.reshape(n, ss, n, ss).mean((1, 3))
    return np.clip(img, 0, 1)


# The settings the committed image was made with.
BUBBLES = dict(count=46, dmax=34, bg=0.11, rim=0.22, glow=0.18, filament=0.14, tendrils=(2, 4),
               bridges=0.55, tendril_value=0.7, fill_gaps=3.0, seed=55)

if __name__ == "__main__":
    img = (orbs_soft(**BUBBLES) * 255).astype(np.uint8)
    Image.fromarray(img, "L").convert("RGB").save(sys.argv[1])
