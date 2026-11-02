# Generates textures/base_trim/proto_comp2.tga, a stand-in for Quake 3's texture of the
# same name: dim red terminal text on black. The proto_comp shader scrolls it upward
# behind a monitor frame, so it tiles seamlessly top to bottom. It's made from scratch,
# never reading the original. The text is drawn large and scaled down, so it blurs like
# a low resolution screen. The committed image is this script's output:
#
#   python3 terminal.py DejaVuSansMono-Bold.ttf textures/base_trim/proto_comp2.tga
#
# It needs numpy, Pillow and DejaVu Sans Mono Bold.
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

LINES = [
    "q3 run --config /config/config.yaml",
    "fetching content from /content-images/0",
    "baseq3/pak0.pk3 ok",
    "Server: q3dm17",
    "------- Game Initialization -------",
    "gamename: baseq3",
    "Sending heartbeat to quake-kube-master",
    "ClientConnect: 1",
    "Daemia entered the game",
    "applied config from the console",
    "",
    "kubectl get quakeservers",
    "NAME        READY   AGE",
    "friday      true    2h",
    "lunch       true    14m",
    "rcon status",
    "map: q3tourney2",
    "num score ping name",
    "  0    12   48 Sarge",
    "  1     9    0 Major",
    "  2     7    0 Daemia",
    "Exit: Fraglimit hit.",
    "",
    "ShutdownGame:",
    "Server: q3tourney2",
    "Loading vm file vm/qagame.qvm",
    "AAS initialized.",
    "Bots added: 3",
]


def render(font_path, n=128, ss=4, size=12, margin=9, color=(255, 30, 24), gain=0.6, seed=1):
    rng = np.random.default_rng(seed)
    N = n * ss
    img = Image.new("RGB", (N, N))
    draw = ImageDraw.Draw(img)
    font = ImageFont.truetype(font_path, size)
    # Spaced so the lines fill the tile exactly, which makes the scroll wrap without a seam.
    step = N / len(LINES)
    for i, line in enumerate(LINES):
        if line:
            draw.text((margin * ss + rng.integers(0, 2) * ss, round(i * step)), line, font=font, fill=color)
    a = np.asarray(img, dtype=np.float32)
    a = a.reshape(n, ss, n, ss, 3).mean((1, 3)) * gain
    return Image.fromarray(np.clip(a, 0, 255).astype(np.uint8))


if __name__ == "__main__":
    render(sys.argv[1]).save(sys.argv[2])
