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

Images for Urban Terror and CPMA are published for x86_64 and aarch64:

- `ghcr.io/chrisrx/quake-kube-urban-terror:4.3.4`
- `ghcr.io/chrisrx/quake-kube-cpma:1.53`

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

Each other entry has a `url` to a pk3 or a zip archive, downloaded each time the
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
browser can't load native game libraries. CPMA and Urban Terror both do. The
browser keeps the files in its cache, but holds all of them in memory while
playing, so a large mod like Urban Terror (1.4 GB) needs a desktop browser.

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

Setting `gateway.presets` replaces the default, which only offers the Quake 3
demo.

## Content that needs the full game

Some mods use files from the full game, which the demo doesn't have and which
can't be freely distributed. Urban Terror is a standalone game, so it doesn't
need any. CPMA's client won't start without the full game's default player
model, mynx, so the CPMA content image builds a stand-in from OpenArena's
"major" model (GPLv2, see `content/cpma/openarena-mynx.sh`). CPMA's maps use
textures from the full game too. cpm4a and cpm29 have all of theirs in the demo
or CPMA, and cpm18r, cpm19, cpm20, cpm22, cpm25, cpm27, cpm28 and cpm3a miss one
or two. Missing textures and sounds don't stop the game, but show up as blank
surfaces and silence.

If you own Quake 3, adding your `pak0.pk3` to `content` with `dir: baseq3`
uses it instead of the demo's, though it can't be redistributed.
