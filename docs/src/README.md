<p align="center">
  <img src="./images/quake-kube-dark.png" width=305 />
  <h1 align="center">QuakeKube</h1>
</p>

QuakeKube is a Kubernetes-ified version of [Quake 3](https://en.wikipedia.org/wiki/Quake_III_Arena). A controller runs dedicated servers for `QuakeServer` objects, along with a master server that lists them, and a web app creates servers and plays them in the browser with [ioquake3](https://github.com/ioquake/ioq3) built for the web with [Emscripten](https://emscripten.org/). It's installed with a Helm chart.

[Try the web app in your browser](try-it.md), with nothing to install.

## What's here

- **[Getting Started](getting-started.md)** installs it on a cluster.
- **[Using The Web App](web-app.md)** covers creating servers, picking maps and playing.
- **[Games](games.md)** lists what it can run out of the box: Quake 3, OpenArena, Urban Terror, CPMA and more.
- **[How It Works](how-it-works.md)** explains the parts and how they fit together.
- **[Configuration](configuration.md)** covers servers and the chart, with the [QuakeServer Reference](quakeserver-reference.md) for every field.
- **[Monitoring And Troubleshooting](troubleshooting.md)** is for when a server doesn't start or a client can't connect.
- **[Development](development.md)** covers working on QuakeKube itself.

## Limitations

Servers use the files from the Quake 3 Demo, which has four maps, rather than the full game. Other maps and mods come from content images or downloads when servers start (see [Add Mods And Maps](add-custom-maps.md)), and maps that use the full game's textures get stand-ins for them. Mods have to ship QVMs to run in the browser, since it can't load native game libraries.

Any non-browser Quake 3 client can connect to the servers too, as long as it has their files.

## What is this project for?

For fun. It started with a hacky version that I deployed as a goof to a work cluster. I wanted to use it for what I called a "Team (Arena) Building" event. What I discovered was a feeling reaching back to old days of networked Doom after work in the office, or lugging around a heavy CRT monitor and computer to a friend's basement for a sleepless night of multiplayer games over LAN. It is a feeling that I think everyone should be free to experience. Quake 3 Arena was a special game and even more so when playing it with people you know. Online matchmaking is convenient but there is nothing like hopping on a server with your buddies/co-workers and playing a Free For All or CTF match.
