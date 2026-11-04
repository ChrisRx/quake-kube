# Try It In Your Browser

**[Open the preview](play/)**

The preview is QuakeKube's web app with nothing to install. You create servers and pick their maps, modes and bots like you would on a cluster, then play them right in the page.

It isn't connected to anything, though:

- Servers are records saved in your browser, up to 3 of them. A game runs in its tab while you play it, against bots.
- There's no Kubernetes, no gateway and no other players. To play with other people, see [Getting Started](getting-started.md).
- It doesn't use Quake 3's files. You can play [OpenArena](https://openarena.ws), a free game built on the same engine, with the [OpenArena Community Mappack](https://github.com/NeonKnightOA/oacmpvol1), [CPMA](https://playmorepromode.com), running on OpenArena's files, or [Urban Terror](https://www.urbanterror.info). See the [Credits](credits.md).
- Creating and deleting a server only look like a pod being deployed and terminated, to show what the real thing is like. A game actually starts when you press Play.

## Browsers

Browsers with [JavaScript Promise Integration](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/Suspending), like Chrome, load game files as the game needs them: about 25 MB before the first map starts, and more as you play. Other browsers download about 420 MB of game files first, and the page says so before you join, and they don't offer Urban Terror, which is too large to download whole. Either way, the files stay in your browser's cache for next time.
