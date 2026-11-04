# QuakeServer Reference

`QuakeServer` is `quake.kube/v1`, namespaced, with the short name `qs`:

```shell
kubectl get qs --all-namespaces
```

Its name is used for a Service and for `<name>-game`, so it has to be a valid
Service name: lowercase letters, numbers and dashes, starting with a letter, up
to 58 characters.

Every field in the spec is optional. The server's cvar is listed where a field
sets one directly.

## Spec

| Field | Type | Default | Cvar | |
|---|---|---|---|---|
| `fragLimit` | integer | 25 | `fraglimit` | Frag limit of maps that don't set their own. |
| `timeLimit` | duration | `15m` | `timelimit` | Time limit of maps that don't set their own, in whole minutes. |
| `bot` | object | | | See [bot](#bot). |
| `game` | object | | | See [game](#game). |
| `server` | object | | | See [server](#server). |
| `maps` | list | q3dm7, q3dm17 | | The rotation, played in order. See [maps](#maps). |
| `commands` | list of strings | | | Console commands run once when the server starts, after the first map loads. |
| `cvars` | map of strings | | any | Set before the first map loads. Names can have letters, numbers and underscores. |
| `content` | list | | | Where the server's files come from. See [content](#content). |

Durations are Go durations, like `90s`, `15m` or `1h30m`.

### bot

| Field | Type | Default | Cvar | |
|---|---|---|---|---|
| `minPlayers` | integer | 0 | `bot_minplayers` | Adds bots until there are this many players. |
| `noChat` | boolean | `true` | `bot_nochat` | Keeps bots from chatting. |

### game

| Field | Type | Default | Cvar | |
|---|---|---|---|---|
| `type` | name or integer | `FreeForAll` | `g_gametype` | Game type of maps that don't set their own. See [game types](#game-types). |
| `fsGame` | string | `baseq3` | `fs_game` | The mod the server runs. |
| `native` | boolean | `false` | | Runs the mod's 32-bit x86 library rather than a QVM. See [Native game code](add-custom-maps.md#native-game-code). |
| `motd` | string | `Welcome to Quake Kube` | `g_motd` | Message of the day. |
| `password` | string | | `g_password` | Password players need to join. It's readable by anyone who can read the QuakeServer. |
| `forceRespawn` | boolean | `false` | `g_forcerespawn` | Respawns dead players without them asking. |
| `inactivity` | duration | `10m` | `g_inactivity` | Idle players are kicked after this long. |
| `quadFactor` | integer | 3 | `g_quadfactor` | How much the Quad Damage multiplies damage. |
| `weaponRespawn` | integer | 3 | `g_weaponrespawn` | Seconds until weapons respawn. |
| `singlePlayerSkill` | integer | 2 | `g_spSkill` | Skill of the bots `minPlayers` adds, from 1 to 5. |

### server

| Field | Type | Default | Cvar | |
|---|---|---|---|---|
| `hostname` | string | the QuakeServer's name | `sv_hostname` | Name shown in server browsers. |
| `maxClients` | integer | 12 | `sv_maxclients` | The most players, including bots. |
| `allowDownload` | boolean | `false` | `sv_allowDownload` | Lets native clients download missing files. |
| `downloadURL` | string | | `sv_dlURL` | Where native clients download them from. |

### maps

| Field | Type | Default | Cvar | |
|---|---|---|---|---|
| `name` | string | required | | The map, without its `.bsp`. |
| `type` | name or integer | `game.type` | `g_gametype` | The map's game type. |
| `fragLimit` | integer | | `fraglimit` | Applies from this map on. |
| `timeLimit` | duration | | `timelimit` | Applies from this map on, in whole minutes. |
| `captureLimit` | integer | | `capturelimit` | Applies from this map on, for any game type. |

### content

Each entry sets one of `url` and `image`. See
[Add Mods And Maps](add-custom-maps.md).

| Field | Type | |
|---|---|---|
| `url` | string | A pk3, or a zip archive of game directories. |
| `image` | string | An OCI image whose top-level directories are game directories. |
| `sha256` | string | The download's checksum in hex. The server doesn't start if it doesn't match. |
| `dir` | string | Where the files go. A pk3 goes in the server's game directory by default, and an archive or image at the top of the content. |
| `stripComponents` | integer | Leading directories removed from paths in an archive. |

### Game types

`type` takes one of Quake 3's game types by name, or a `g_gametype` number, since
mods number their own:

| Name | Number |
|---|---|
| `FreeForAll` | 0 |
| `Tournament` | 1 |
| `SinglePlayer` | 2 |
| `TeamDeathmatch` | 3 |
| `CaptureTheFlag` | 4 |

## Status

| Field | |
|---|---|
| `ready` | Whether the server's pod is running its current spec. Browser clients can join then. |
| `address` | Where native clients connect, as `host:port`, once it has one. |
| `port` | The external port allocated from `gameServers.exposure.portRange`. |
| `message` | Why the server isn't running, when it's stuck rather than still starting. |
| `observedGeneration` | The spec's generation the status describes. |

## How changes apply

| What changed | What happens |
|---|---|
| `game.fsGame`, `game.native` or `content` | The pod is replaced, which disconnects everyone. |
| `commands` | Nothing until the server next starts, since they only run then. |
| Anything else | `q3 run` applies it from the server's console and reloads the map. Players stay connected. |

Changes made with `kubectl` reach the server within about a minute, since the
Secret has to reach the pod. Changes from the web app apply right away (see
[Changing a server](how-it-works.md#changing-a-server)).

## What the controller creates

Each of these has the server's name unless it says otherwise. They're owned by
the QuakeServer, so deleting it deletes them.

| Object | |
|---|---|
| Secret | `config.yaml`, the rendered configuration, and `rconPassword` |
| Deployment | One replica, which stops the old pod before starting a new one |
| Service | Ports `http` (8080) and `game` (27960/UDP), for use inside the cluster |
| Service `<name>-game` | The game's port for native clients, with `LoadBalancer` or `NodePort` exposure |

They're labeled `app.kubernetes.io/name=quake-server` and
`app.kubernetes.io/instance=<name>`. The pod has a `server` container, and a
`content` init container when the server has content.

## Labels and annotations

The gateway keeps these on servers created from the web app:

| Key | Kind | |
|---|---|---|
| `quake.kube/created-by` | Label | `gateway`, for servers the web app can change and delete |
| `quake.kube/preset` | Label | The preset the server was created from |
| `quake.kube/game-type` | Annotation | The name of the preset's game type |
| `quake.kube/owner` | Annotation | The email address of whoever created it, with [authentication](authentication.md) |
| `quake.kube/idle-since` | Annotation | Since when nobody has been on the server, removed when someone joins |
