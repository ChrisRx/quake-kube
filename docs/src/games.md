# Games

The chart's default presets offer these games in the web app. All of them run
in the browser, and native clients can join them too, as long as they have the
game's files.

| Game | Preset | Modes | Maps | Bots |
|---|---|---|---|---|
| Quake 3 | `quake3` | Free For All, Tournament, Team Deathmatch | 48 | Yes |
| Quake 3 CTF | `quake3-ctf` | Capture The Flag | 3 | Yes |
| [OpenArena](https://openarena.ws) | `openarena` | Free For All, Tournament, Team Deathmatch, Elimination, Last Man Standing | 63 | Yes |
| OpenArena CTF | `openarena-ctf` | Capture The Flag, One Flag CTF, Overload, Harvester, CTF Elimination, Double Domination | 16 | Yes |
| [Urban Terror 4.3](https://www.urbanterror.info) | `urban-terror` | Capture The Flag, Team Deathmatch, Team Survivor, Free For All, Bomb Mode | 13 | No |
| [CPMA](https://playmorepromode.com) | `cpma` | Free For All, Duel, Team Deathmatch, Clan Arena | 12 | Yes |
| Rocket Arena | `rocket-arena` | Clan Arena | 6 | No |
| Rocket Arena 3 | `rocket-arena-3` | Arenas | 6 | No |
| [Ultra Freeze Tag](https://lvlworld.com/mod/id:39) | `ufreeze` | Team Freeze, Team Freeze With Grappling Hook | 48 | Yes |
| [DeFRaG](https://q3defrag.org) | `defrag` | Vanilla Physics, CPM Physics | 5 | No |

## Kubernetes versions

Every preset gets its files from [content images](add-custom-maps.md#content-images),
which need image volumes: Kubernetes 1.35, or 1.33 with the `ImageVolume` feature
gate. On older clusters, the chart leaves out every preset except Quake 3, which
is left with the demo's four maps: q3dm1, q3dm7, q3dm17 and q3tourney2.

`gateway.imagePresets` overrides this (see
[Presets](add-custom-maps.md#presets)).

## Notes on each game

**Quake 3** servers use the files from the Quake 3 demo, along with
[47 community maps](credits.md#community-maps). Stand-ins replace the textures
those maps use from the full game, which the demo doesn't have, so some surfaces
look different from how their authors made them (see
[Content that needs the full game](add-custom-maps.md#content-that-needs-the-full-game)).

**OpenArena** is a free game built on the same engine. Its presets also offer
the Quake 3 demo's maps and the community maps. Native OpenArena clients don't
have the demo's files, though browsers download them.

**Urban Terror** is a standalone game with no bots. Its files are around 1.5 GB,
so a browser without [JSPI](add-custom-maps.md#loading-in-the-browser) needs to
hold all of them in memory.

**CPMA** chooses its modes with its `mode_start` cvar rather than `g_gametype`,
and needs `server_useMapModes` set to 0 to use it. The preset does both.

**Rocket Arena** plays Rocket Arena 3's maps on CPMA, whose Clan Arena is the
same game mode with no native code. These maps have no bot navigation files, so
it has no bots.

**Rocket Arena 3** is the mod itself. Its server code is a closed source 32-bit
x86 library, so these servers only run on x86-64 nodes (see
[Native game code](add-custom-maps.md#native-game-code)).

**DeFRaG** is a racing and trick jumping mod. Its modes choose the physics.

## Your own games

Setting `gateway.presets` replaces all of these. See
[Presets](add-custom-maps.md#presets) for writing your own, and
[Add Mods And Maps](add-custom-maps.md) for what a server can run.
