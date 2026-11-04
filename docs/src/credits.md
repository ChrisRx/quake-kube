# Credits

* [ioquake/ioq3](https://github.com/ioquake/ioq3) - The community supported version of Quake 3 used for the dedicated server and the browser client. It is licensed under the GPLv2.
* [inolen/quakejs](https://github.com/inolen/quakejs) - The really awesome QuakeJS project, which was the original browser client and showed it could be done.
* [begleysm/quakejs](https://github.com/begleysm/quakejs) - Information in the README.md (very helpful) was used as a guide for setting up the original QuakeJS client.
* [OpenArena](https://openarena.ws) - Its "major" player model and voice stand in for CPMA's default player model, which is only in the full game, and its textures and models stand in for the full game's in maps and mods. Its game and the [OpenArena Community Mappack](https://github.com/NeonKnightOA/oacmpvol1) are what the [browser preview](try-it.md) runs. It is licensed under the GPLv2.
* [Missing textures pack](http://download.tuxfamily.org/openarena/files/pk3/missingtextures/) - Gábor Udvari's replacements for missing Quake 3 textures, made for OpenArena. A few stand in for the full game's in community maps. It is licensed under the GPLv2.
* [q3a2oa textures, version 4](http://download.tuxfamily.org/openarena/files/pk3/q3a2oa/) - Jack Thompson's placeholders for Quake 3 textures, each a block of its average color. They stand in where nothing better has been found. It is licensed under the GPLv2.
* The rest of the community maps' stand-ins were made for QuakeKube: some generated from scratch, like the lava's bubbles in ChiropteraTA, and one adding rust to OpenArena's `dirty_pewter_big`. Their scripts are in `content/quake3-maps/standins/generated`, and how each stand-in was chosen is recorded in `content/quake3-maps/standins/decisions.jsonl`.
* [CPMA](https://playmorepromode.com) - Challenge ProMode Arena, by the CPMA team, which the [browser preview](try-it.md) offers.
* [Urban Terror](https://www.urbanterror.info) - By FrozenSand, which the [browser preview](try-it.md) offers. The `ut4_orbital_sl` map is by Kr0t4L.
* Rocket Arena 3 - The round-based arena mod. Its readme ships with it in the `ghcr.io/chrisrx/quake-kube-rocket-arena` image.
* [Ultra Freeze Tag](https://lvlworld.com/mod/id:39) - By Neil "haste" Toronto, built on Darrell Bircsak's Freeze Tag.
* [DeFRaG](https://q3defrag.org) - The trick jumping and racing mod, by the DeFRaG team.
* [joz3d.net](http://www.joz3d.net/html/q3console.html) - Useful information about configuration values.

## Community maps

The Quake 3 community maps come from [lvlworld](https://lvlworld.com), and their
readmes allow distributing them for free, unaltered. They're shipped as they
were released, with their readmes, in the
`ghcr.io/chrisrx/quake-kube-quake3-maps` image. Bundling them there, rather than
downloading them from lvlworld when servers start, keeps QuakeKube from putting
load on a site that has kept these maps available for decades: game servers get
them from the image, and players' browsers from the game server.

| Map | Author | Name | |
|---|---|---|---|
| Q3Deck | kinetix | `q3deck` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20q3deck&content=q3deck) |
| ChiropteraTA | Alcatraz, nunuk and Sock | `chiropterata` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20chiropterata&content=chiropterata) |
| Desert Temple | LordSquart | `lsdm1` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20lsdm1&content=lsdm1) |
| Estatica | Cardigan | `estatica` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20estatica&content=estatica) |
| Trial by Error | R.P.G. | `rpg3dm2` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20rpg3dm2&content=rpg3dm2) |
| You'll Shoot Your Eye Out | Lunaran and KungFuSquirrel | `lun3dm5` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20lun3dm5&content=lun3dm5) |
| Evolution | thefury | `pukka3tourney2` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20pukka3tourney2&content=pukka3tourney2) |
| suboptimal | thefury | `pukka3tourney5` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20pukka3tourney5&content=pukka3tourney5) |
| The Hot Place | thefury | `pukka3dm1` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20pukka3dm1&content=pukka3dm1) |
| Tortured | thefury | `pukka3tourney7` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20pukka3tourney7&content=pukka3tourney7) |
| ALIEN | AKUTA | `akutadm1` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20akutadm1&content=akutadm1) |
| Saxa Liquefacta | AKUTA | `akutadm2` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20akutadm2&content=akutadm2) |
| Magnetic Centrifuge | AKUTA | `akutadm3` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20akutadm3&content=akutadm3) |
| Core Gravity Drive | AKUTA | `akutatourney5` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20akutatourney5&content=akutatourney5) |
| Deep Purple | AKUTA | `akutatourney6` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20akutatourney6&content=akutatourney6) |
| Running Man | AKUTA | `akutatourney8` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20akutatourney8&content=akutatourney8) |
| Terminatria | bst | `bst3dm1` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20bst3dm1&content=bst3dm1) |
| Rainy Day | cityy | `ct3tourney2` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20ct3tourney2&content=ct3tourney2) |
| TNK gas station | Anton | `gas-st` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20gas-st&content=gas-st) |
| Ballistophobia | Jester | `jstrq3dm2` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20jstrq3dm2&content=jstrq3dm2) |
| Inner Sanctum | kat | `katdm3` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20katdm3&content=katdm3) |
| Winter Games | Comic Relief | `wintergames` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20wintergames&content=wintergames) |
| Hypersonic Tourney | zeal0r | `zl3tourney1` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20zl3tourney1&content=zl3tourney1) |
| Hot and Moist | pjw | `pjw3tourney1` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20pjw3tourney1&content=pjw3tourney1) |
| Hydra | Pat Howard | `hydra` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20hydra&content=hydra) |
| Goldleaf | Pat Howard | `goldleaf` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20goldleaf&content=goldleaf) |
| Endurance | flipout | `q3gwdm2` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20q3gwdm2&content=q3gwdm2) |
| Windsong Keep | Phantazm11 | `phantq3dm4` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20phantq3dm4&content=phantq3dm4) |
| Corrosion | Phantazm11 | `phantq3dm3_rev` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20phantq3dm3_rev&content=phantq3dm3_rev) |
| Pyramid of the Magician | Sock | `pom_bots` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20pom_bots&content=pom_bots) |
| Dynasty | EmeraldTiger | `pasdm1` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20pasdm1&content=pasdm1) |
| Tatatu | GrindSpire | `gs3dm5` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20gs3dm5&content=gs3dm5) |
| The Forlorn Hope | JustOneFiX | `jof3dm2` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20jof3dm2&content=jof3dm2) |
| Grindill 2.0 | Quaker-X | `qxdm4_v2` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20qxdm4_v2&content=qxdm4_v2) |
| Left Behind | cityy | `ct3dm5` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20ct3dm5&content=ct3dm5) |
| Revenga! | Strahlemann | `revenga` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20revenga&content=revenga) |
| PowerStroke | AKUTA | `akutatourney3` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20akutatourney3&content=akutatourney3) |
| Military Complex | AKUTA | `akutatourney4` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20akutatourney4&content=akutatourney4) |
| Deep Freeze | Shadowdane | `shad3dm2` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20shad3dm2&content=shad3dm2) |
| Blood Run Tourney | ztn | `ztn3tourney1` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20ztn3tourney1&content=ztn3tourney1) |
| Derwyll's Castle 2 | Noruen | `nor3ctf1` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20nor3ctf1&content=nor3ctf1) |
| Substation 11 | Phantazm11 | `phantq3ctf1_rev` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20phantq3ctf1_rev&content=phantq3ctf1_rev) |
| Geotechnic | Phantazm11 | `phantq3dm6_mc` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20phantq3dm6_mc&content=phantq3dm6_mc) |
| Achromatic | flipout | `q3gwdm1` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20q3gwdm1&content=q3gwdm1) |
| The Edge of Forever | Sock | `moteof_final` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20moteof_final&content=moteof_final) |
| Trespass | Pat Howard | `trespass` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20trespass&content=trespass) |
| Ironwood | Pat Howard | `ironwood` | [Request removal](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml&title=Content%20removal%3A%20ironwood&content=ironwood) |

## Content removal

Most of the maps and mods QuakeKube ships were made years ago by people who can't
be reached anymore, and their licenses are often loosely worded. They're included
because their readmes allow distributing them, with every effort to respect their
authors' wishes. If you made something QuakeKube ships and want it removed, or its
credit changed, [open a content removal request](https://github.com/ChrisRx/quake-kube/issues/new?template=content-removal.yml), or use the link next to
a map above, and it will be taken out.
