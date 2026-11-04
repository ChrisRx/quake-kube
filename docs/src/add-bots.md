# Add bots

The simplest way to add bots is to set a minimum number of players. The server
adds bots up to that number and removes them as people join:

```yaml
spec:
  bot:
    minPlayers: 8
  game:
    singlePlayerSkill: 2
```

`singlePlayerSkill` sets the skill of the bots it adds, from 1 to 5 (2 by
default). The web app's **Bots** and **Bot skill** fields set them for servers it
creates. Changing the skill on a running server replaces its bots with new ones,
unless `commands` added them.

Bots can also be added individually with the `addbot` command, which takes the
bot's name and skill level, using the `commands` section:

```yaml
spec:
  commands:
    - addbot crash 1
    - addbot sarge 2
```

Commands only run when the server starts, so changing them doesn't add bots to a
running server.

The Quake 3 demo only has some of the game's bots. Mods like Urban Terror don't
have bots at all, so their presets hide the web app's **Bots** field. Bots also need
a map's `.aas` navigation file, which some maps don't have.
