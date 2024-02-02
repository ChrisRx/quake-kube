# Add bots

Bots can be added individually to map rotations using the `commands` section of the config:

```yaml
commands:
  - addbot crash 1
  - addbot sarge 2
```

The `addbot` server command requires the name of the bot and skill level (crash and sarge are a couple of the built-in bots).

Another way to add bots is by setting a minimum number of players to allow the server to add bots up to a certain value (removed when human players join):

```yaml
bot:
  minPlayers: 8
game:
  singlePlayerSkill: 2
```

`singlePlayerSkill` can be used to set the skill level of the automatically added bots (2 is the default skill level).
