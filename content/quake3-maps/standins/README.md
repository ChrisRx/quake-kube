# Stand-ins

Many community maps use textures from the full Quake 3, which aren't in the demo content
QuakeKube servers have. Without them, those surfaces show the engine's gray grid. The
content image ships stand-ins for them in `baseq3/zz-quake3-maps-openarena-standins.pk3`,
which loads after the maps. None of them are id's textures: they're GPL replacements made
for OpenArena, look-alikes picked from those, or textures made for QuakeKube.

## What's in the stand-in pak

Each layer can replace the one before it:

| Layer | File | What |
| --- | --- | --- |
| Same names | `images.txt`, `standins.shader` | OpenArena's replacements under the names maps want, found by `hack/standins.py`. |
| Flat colors | `flat.txt` | Placeholders from Jack Thompson's `z-q3a2oa-textures-v4-080.pk3` (GPL): each texture as a block of its average color, so nothing shows the gray grid. |
| Look-alikes | `renamed.txt` | Textures from OpenArena's paks, or from Gábor Udvari's `z_missing-textures1_v1.pk3` (GPL v2), under the name a map wants. |
| Made for QuakeKube | `generated/` | Images made by the scripts beside them, e.g. `orbs.py` for `liquids/bubbles`. Each script's header says how to run it, and its output is committed. `rust.py` modifies OpenArena's `dirty_pewter_big`, so that one's a derivative of a GPL texture. |

Udi's and Jack Thompson's paks are kept in `sources/`, with their GPL licenses inside them,
since sites hosting Quake 3 content keep disappearing. `default.nix` notes where each came
from. The larger downloads, OpenArena and the maps, are pinned in the project's Cachix
cache, so builds keep working if they disappear upstream.

Every surface that needs a judgment call has a line in `decisions.jsonl`, the record of what
was decided and why. The latest line for a texture is what counts:

```json
{"time": "...", "target": "textures/base_support/metal3_3", "kind": "pick", "pick": "pak4-textures.pk3 textures/gothic_trim/metalsupport4i_bit.jpg", "note": "Very close, lacks the faint whiteness around the border but I like it better", "comment": "q3deck, q3gwdm1: riveted metal plate"}
```

`kind` is `pick`, `generated`, `flat` (a placeholder) or `none`. `hack/sweep/apply.py` writes
`renamed.txt` and `flat.txt` from it, so don't edit those by hand.

## Revisiting a texture

The picker lists every texture that needs a judgment call, most used first, marked with what
was decided: ✓ picked, ★ generated, ▪ flat color placeholder, – no stand-in, · open. It shows
the retail original beside every candidate from OpenArena and Udi's pak, ranked by
likeness, and records what you decide in `decisions.jsonl`.

1. Start it with the retail game's `baseq3/pak0.pk3`, which ranks the candidates:

   ```sh
   task content:sweep PAK0=~/quake3/baseq3/pak0.pk3
   ```

   and open http://127.0.0.1:8765/. The first run takes several minutes, making images of
   every candidate in `.cache/sweep`.
2. Pick a replacement, use the flat color, or say there's none. "Only open, placeholders
   and no stand-in" in the sidebar hides what's settled.
3. Write the lists, and bump the content's version (`version` in `default.nix`, and the
   `quake3-maps` image tag in `chart/values.yaml` and `hack/k3d/presets.yaml`):

   ```sh
   python3 -I hack/sweep/apply.py content/quake3-maps/standins
   ```

To change a generated one, like the bubbles, change its script's settings and run the command
in its header. To replace one with a pick, decide it in the picker and remove its image from
`generated/`, which `apply.py` reminds you to do.

## Keeping it clean

The retail `pak0.pk3` is only for ranking and checking. It and what's extracted from it stay
out of git. Before a stand-in goes in, check it isn't derived from the original:

```sh
python3 -I hack/sweep/check.py <stand-in> .cache/sweep/references/<its path>
```

Textures drawn independently score near 0. Copies, even blurred or recolored ones, score high.
That's how `z-gpl-q3a2oa-textures-v5` was found to be made from id's textures, even though it
claims the GPL, so nothing comes from it. Generated textures are made from descriptions, never
from the originals.

What's still open shows in the picker without a mark: akutadm1's alien model, whose textures
nothing covers, a texture pukka3tourney2 forgot to ship, and names in q3gwdm1 that aren't
images at all.
