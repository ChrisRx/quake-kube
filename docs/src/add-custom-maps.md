# Add mods and maps

A `QuakeServer`'s `content` lists where its files come from, in addition to the
Quake 3 demo. They're put in place when the server's pod starts, before the game
server runs.

## Content images

An entry with `image` names an OCI image whose top-level directories are game
directories, e.g. `q3ut4/`. The image is mounted in the pod as an
[image volume](https://kubernetes.io/docs/tasks/configure-pod-container/image-volumes/)
and its files are linked into the content, so nothing is downloaded once the
node has the image, and servers on the same node share it. Image volumes need
Kubernetes 1.35, or 1.33 with the `ImageVolume` feature gate.

```yaml
content:
  - image: ghcr.io/chrisrx/quake-kube-urban-terror:4.3.4
```

These images are published for x86_64 and aarch64:

| Image | Content | Directory |
|---|---|---|
| `ghcr.io/chrisrx/quake-kube-urban-terror:4.3.4` | Urban Terror 4.3.4, and the community map ut4_orbital_sl | `q3ut4` |
| `ghcr.io/chrisrx/quake-kube-cpma:1.53` | CPMA 1.53 and its map pack | `cpma` |
| `ghcr.io/chrisrx/quake-kube-rocket-arena:1.76` | Rocket Arena 3, and its maps for CPMA's Clan Arena | `arena`, `cpma` |
| `ghcr.io/chrisrx/quake-kube-openarena:0.8.8` | OpenArena 0.8.8 and the OpenArena Community Mappack Vol. 1 | `baseoa` |
| `ghcr.io/chrisrx/quake-kube-quake3-maps:6` | 47 community maps for Quake 3 from lvlworld (see [Credits](credits.md#community-maps)) | `baseq3` |
| `ghcr.io/chrisrx/quake-kube-ufreeze:1.21` | Ultra Freeze Tag 1.21 | `ufreeze` |
| `ghcr.io/chrisrx/quake-kube-defrag:1.91.31` | DeFRaG 1.91.31 and its test maps | `defrag` |

They're built from `content/` by the flake. To build one yourself and push it
to your own registry:

```shell
nix build .#urban-terror-image
docker load < result
docker tag quake-kube-urban-terror:latest registry.example.com/quake-kube-urban-terror:4.3.4
docker push registry.example.com/quake-kube-urban-terror:4.3.4
```

A content image is any image with game directories at its top, so one with
other mods or maps can be built however you like, e.g. from a Dockerfile:

```dockerfile
FROM scratch
COPY q3ut4 /q3ut4
```

## Downloads

An entry with `url` names a pk3 or a zip archive, downloaded each time the
server's pod starts:

- A pk3 goes in the server's game directory, `game.fsGame` or `baseq3`, unless
  `dir` says otherwise.
- An archive is extracted at the top of the content, where each game has its
  own directory (`baseq3/`, `cpma/`, ...), so a mod's release that contains
  `cpma/z-cpma-pak153.pk3` lands where the game looks for it. Use `dir` to
  extract it somewhere else, and `stripComponents` to remove leading
  directories from its paths. Files left outside a directory are skipped.

Setting `sha256` makes the server refuse a download that doesn't match.

Changing `content` replaces the server's pod.

## Mods

`game.fsGame` picks the mod the server runs, and `cvars` sets anything that has
to be in place before the first map loads:

```yaml
apiVersion: quake.kube/v1
kind: QuakeServer
metadata:
  name: cpma
spec:
  game:
    fsGame: cpma
  cvars:
    mode_start: FFA
    # Otherwise CPMA picks each map's mode from its own list.
    server_useMapModes: "0"
  content:
    - url: https://cdn.playmorepromode.com/files/cpma/cpma-1.53-nomaps.zip
      sha256: edfffa0c1a0375ba46a5b42257a168fb15086712245733526ab2d9ccdd821ca0
    - url: https://cdn.playmorepromode.com/files/cpma-mappack-full.zip
      dir: cpma
  maps:
    - name: cpm22
```

Mods number their own game types, so `type` also takes a `g_gametype` number,
e.g. `7` for Capture The Flag in Urban Terror. The names (`FreeForAll`,
`CaptureTheFlag`, ...) are the standard Quake 3 game types.

Browser clients download the server's content, so the mod has to ship QVMs: the
browser can't load native game libraries. CPMA and Urban Terror both do.

### Native game code

Browsers need a mod's game code as QVMs, and so do servers normally. Rocket
Arena 3 ships QVMs for the client, but its server code is only a 32-bit x86
library. With `game.native`, the server runs a 32-bit build of the game server
that loads it:

```yaml
spec:
  game:
    fsGame: arena
    native: true
```

Such servers only run on x86-64 nodes, which the controller selects. The
library is closed source native code from 2004, and runs inside the server's
pod with an executable stack, which it needs. CPMA's Clan Arena is the same game
mode with no native code, and the Rocket Arena preset plays RA3's maps with it.

### Loading in the browser

Browsers with WebAssembly's JSPI, such as Chrome and Edge, load files from paks
as the game reads them, rather than downloading whole paks. A map in Urban
Terror reads around 150 MB of its 1.5 GB. If the content has a profile, which
lists the files the game reads on each map, the play page fetches those in a
few large batches before the game starts, and the game only waits for anything
the profile missed. Browsers keep what they fetch in their cache, so joining
again, or joining another server with the same content, fetches only what's new.

Other browsers download paks before the game starts and hold them in memory
while playing, so a large mod like Urban Terror needs a desktop browser there.
They only download a map's own pak when the server plays that map, or when the
profile says the map reads from it, and fetch the paks of the other maps the
server rotates through in the background after joining. A map's pak is one
named after every map in it, like `map_cpm22.pk3`. Paks that bundle maps under
other names, like Urban Terror's, often carry shared content too, so browsers
always download them. The play page says how much joining downloads, and
browsers keep the files in their cache.

### Profiles

A content image's profile is `<game>/quake-kube-profile.json`, e.g.
`cpma/quake-kube-profile.json`. Most of the published images have one,
generated by playing each of their preset's maps in the dev cluster (see
[Development](development.md#profiles)). A server merges the profiles of all its
content, so a map pack can bring its own.

## Presets

The web app creates servers from the presets in the chart's `gateway.presets`.
Each one is a game: its game types, its maps, whether it has bots, and the
start of each server's spec, such as a mod's `game.fsGame` and `content`. A game
type sets `type`, `cvars` or both, since mods choose game types differently:

```yaml
gateway:
  presets:
    - name: urban-terror
      title: Urban Terror 4.3
      bots: false
      gameTypes:
        - {name: Capture The Flag, type: 7}
        - {name: Team Deathmatch, type: 3}
      maps: [ut4_turnpike, ut4_casa]
      spec:
        game: {fsGame: q3ut4}
        content:
          - image: ghcr.io/chrisrx/quake-kube-urban-terror:4.3.4
    - name: cpma
      title: CPMA
      gameTypes:
        - {name: Free For All, cvars: {mode_start: FFA}}
        - {name: Duel, cvars: {mode_start: 1v1}}
      maps: [cpm22, cpm4a]
      spec:
        game: {fsGame: cpma}
        cvars: {server_useMapModes: "0"}
        content:
          - image: ghcr.io/chrisrx/quake-kube-cpma:1.53
```

These are in the chart's default presets, with more of their game types and
maps, alongside the [other games](games.md). Setting `gateway.presets` replaces
them all, so copy the ones you want to keep from the chart's
[values.yaml](https://github.com/ChrisRx/quake-kube/blob/main/chart/values.yaml).

A preset has these fields:

| Field | |
|---|---|
| `name` | Identifies the preset, e.g. `urban-terror`. |
| `title` | Shown in the web app, e.g. `Urban Terror 4.3`. |
| `gameTypes` | The modes offered. Each has a `name`, and optionally a `type`, `cvars` and `arena`. |
| `maps` | The maps to pick from, which must be in the preset's content. |
| `bots` | Whether the game has bots, `true` by default. |
| `spec` | The start of each server's [spec](quakeserver-reference.md#spec). The web app fills in the name, game type, maps, bots, password and limits. |
| `mapLists` | Lists of maps to suggest, each with a `name` and `maps`. |
| `mapsWithoutImages` | The maps to offer on clusters that can't mount content images. |

Servers play a rotation of the preset's maps, picked in the web app. The picker
offers the maps made for the chosen game type first, from each map's arena file
(e.g. `scripts/q3dm17.arena`). It knows Quake 3's game types; for a mod's, set
`arena` to the type its maps' arena files use, e.g. `ctf` or `team`. Presets can
also suggest lists of maps to pick from with `mapLists`:

```yaml
gateway:
  presets:
    - name: openarena-ctf
      title: OpenArena CTF
      gameTypes:
        - {name: Capture The Flag, type: 4, arena: ctf}
        - {name: One Flag CTF, type: 5, arena: oneflag}
      maps: [oasago2, ps37ctf2, oa_ctf4ish]
      mapLists:
        - name: Classics
          maps: [oasago2, ps37ctf2]
```

People can save their own lists in their browser too, and share a rotation with
a link from the create dialog.

Presets with content images need image volumes, so on clusters older than
Kubernetes 1.35 the chart leaves them out. Set `gateway.imagePresets` to `true`
on 1.33 or 1.34 with the `ImageVolume` feature gate, or to `false` to never
offer them. A preset with `mapsWithoutImages` is kept without its images
instead, offering only those maps, like the Quake 3 preset, which offers just
the demo's maps then.

Maps for Quake 3 also play in OpenArena, which runs on top of Quake 3's
`baseq3`, so the demo's maps and the Quake 3 community maps image are in the
OpenArena preset too. Native OpenArena clients don't have the demo's files,
though browsers download them.

## Content that needs the full game

Some mods use files from the full game, which the demo doesn't have and which
can't be freely distributed. Urban Terror is a standalone game, so it doesn't
need any. CPMA's client won't start without the full game's default player
model, mynx, so the CPMA content image builds a stand-in from OpenArena's
"major" model (GPLv2, see `content/cpma/openarena-mynx.sh`).

Maps often use textures from the full game too. OpenArena has same-named
replacements for many of them, so content images include a pk3 of the ones their
maps need, built by `content/openarena-standins.sh` from a list that
`hack/standins.py` writes:

```shell
python3 -I hack/standins.py content/cpma/standins \
  --base <demo paks> <the mod's paks> \
  --source <OpenArena's baseoa paks> \
  --maps <map pk3s>
```

It reports what's still missing. The maps in the presets miss at most a few
textures, and only Rocket Arena 3's maps that are complete, or close to it, are
included. Missing textures and sounds don't stop the game, but show up as blank
surfaces and silence.

If you own Quake 3, adding your `pak0.pk3` to `content` with `dir: baseq3`
uses it instead of the demo's, though it can't be redistributed.
