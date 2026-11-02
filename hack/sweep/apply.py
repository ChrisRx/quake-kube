"""Writes the stand-in lists from decisions.jsonl, where the latest line for each texture is
what's decided for it:

  pick       a file from one of the paks, under the texture's name: standins/renamed.txt
  flat       its block of average color from z-q3a2oa-textures-v4-080.pk3: standins/flat.txt
  generated  made by a script in standins/generated, whose output is committed
  none       nothing stands in for it

Usage: python3 -I apply.py <standins dir>
"""
import json
import os
import sys

RENAMED_HEADER = """\
# Stand-ins under other names, for the full game's textures that OpenArena
# has no same-named replacement for: "<pak> <its file> <name a map wants>",
# with the closest look-alike. Paks are OpenArena's, or
# z_missing-textures1_v1.pk3, Gábor Udvari's GPL replacements. Written by
# hack/sweep/apply.py from decisions.jsonl.
"""

FLAT_HEADER = """\
# Placeholders: Quake 3 textures nothing better stands in for yet, as blocks of their
# average color from Jack Thompson's z-q3a2oa-textures-v4-080.pk3. One path per line, as
# in that pak. Written by hack/sweep/apply.py from decisions.jsonl.
"""


def main(standins):
    latest = {}
    for line in open(os.path.join(standins, "decisions.jsonl")):
        if line.strip():
            d = json.loads(line)
            # Keep each texture where it was first decided, so the lists stay in order.
            latest[d["target"]] = {**latest.get(d["target"], {}), **d}
    renamed, flat = [RENAMED_HEADER], [FLAT_HEADER]
    for target, d in latest.items():
        # Generated stand-ins are copied in last, so one left over would win over the decision.
        for ext in (".tga", ".jpg", ".png"):
            leftover = os.path.join(standins, "generated", target + ext)
            if d["kind"] != "generated" and os.path.exists(leftover):
                sys.exit(f"{target} is decided as {d['kind']}, but {leftover} would replace it: remove it")
        if d["kind"] == "pick":
            pak, src = d["pick"].split()[:2]
            if " quarter " in d["pick"]:
                sys.exit(f"{target}: a quarter of {src} needs cropping, which only a script in generated can do")
            ext = os.path.splitext(src)[1]
            if d.get("comment"):
                renamed.append(f"# {d['comment']}\n")
            renamed.append(f"{pak} {src} {target}{ext}\n")
        elif d["kind"] == "flat":
            flat.append(d["pick"] + "\n")
        elif d["kind"] == "generated":
            path = os.path.join(standins, "generated", d["pick"])
            if not os.path.exists(path):
                sys.exit(f"{target}: {path} doesn't exist")
    open(os.path.join(standins, "renamed.txt"), "w").write("".join(renamed))
    open(os.path.join(standins, "flat.txt"), "w").write("".join(flat))


if __name__ == "__main__":
    main(sys.argv[1])
