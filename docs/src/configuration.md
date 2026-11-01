# Configuration

## QuakeServer

Each server is a `QuakeServer`. Every field is optional:

```yaml
apiVersion: quake.kube/v1
kind: QuakeServer
metadata:
  name: quake
spec:
  # Defaults for maps that don't set their own.
  fragLimit: 25
  timeLimit: 15m
  bot:
    minPlayers: 4
    noChat: true
  game:
    # Game type of maps that don't set their own: FreeForAll, Tournament,
    # SinglePlayer, TeamDeathmatch or CaptureTheFlag, or a mod's g_gametype
    # number.
    type: FreeForAll
    motd: "Welcome to Quake Kube"
    password: "letmein"
    forceRespawn: false
    # Idle players are kicked after this long.
    inactivity: 10m
    quadFactor: 3
    # Seconds.
    weaponRespawn: 3
    singlePlayerSkill: 2
  server:
    # Defaults to the QuakeServer's name.
    hostname: "Quake Kube"
    maxClients: 12
  # The map rotation, played in order.
  maps:
    - name: q3dm7
    - name: q3dm17
      fragLimit: 30
      timeLimit: 20m
    - name: q3tourney2
      type: Tournament
  # Console commands run once the first map has loaded.
  commands:
    - addbot sarge 2
  # Cvars set before the first map loads.
  cvars:
    sv_fps: "30"
```

Without `maps`, the server plays q3dm7 and q3dm17. The Quake 3 demo only has
q3dm1, q3dm7, q3dm17 and q3tourney2. Others need content (see
[Add Mods And Maps](add-custom-maps.md)), which is also where `game.fsGame` and
`content` are covered.

A map's `fragLimit` and `timeLimit` apply from that map on, and Capture The Flag
maps can set a `captureLimit`:

```yaml
maps:
  - name: q3wctf3
    type: CaptureTheFlag
    captureLimit: 8
```

Changes to the spec are applied to the running server, which restarts with the
new configuration. Changing the content or the mod replaces the server's pod.

## The chart

The chart's [values.yaml](https://github.com/ChrisRx/quake-kube/blob/main/chart/values.yaml)
documents its settings, including:

- `gateway.presets`: the games the web app offers (see
  [Add Mods And Maps](add-custom-maps.md#presets))
- `gateway.maxServers`: how many servers the web app can create, or 0 to turn
  creating servers off
- `gameServers.exposure`: how native clients reach game servers, and the port
  range servers get their external ports from when they share an address
- `master.service`: how native clients reach the master server
