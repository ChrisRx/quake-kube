// Runs the web app without a gateway, for the preview on the docs site. hack/play-site builds
// the pages to load this first. It answers the pages' requests in place of the gateway: servers
// are records kept in the browser, and playing one starts a local game with bots in the page.
// The games don't use Quake 3's files, so its license isn't involved.
"use strict";

{
  const root = new URL("..", document.currentScript.src).pathname.replace(/\/$/, "");
  // A game too large to download whole needs a browser that can read parts of its paks.
  const presets = globalThis.previewPresets.filter((p) => !p.onDemand || "Suspending" in WebAssembly);
  const STORE = "previewServers";
  const NAMESPACE = "browser";
  const MAX_SERVERS = 3;
  const MAX_NAME = 64;
  const MAX_ROTATION = 32;
  const CONFIG = "quake-kube-preview.cfg";
  // Ranges closer than this are fetched as one, since a request costs more than the gap.
  const RANGE_GAP = 64 * 1024;
  const RANGE_CONCURRENCY = 6;
  // What a new server appears to go through, like a pod being deployed, with how long each
  // takes in milliseconds. Nothing is really happening: games start when they're played.
  const PHASES = [
    ["scheduling", 1500],
    ["pulling image", 4000],
    ["creating container", 1500],
    ["starting", 2000],
  ];
  // How long a deleted server appears to take to stop, in milliseconds.
  const TERMINATING = 4000;

  // Used when the browser won't store anything, so the page still works until it's closed.
  let unsaved = [];
  function load() {
    let servers;
    try {
      servers = JSON.parse(localStorage.getItem(STORE)) ?? [];
    } catch {
      servers = unsaved;
    }
    return servers.filter(({ deletedAt }) => !deletedAt || Date.now() - deletedAt < TERMINATING);
  }
  function save(servers) {
    unsaved = servers;
    try {
      localStorage.setItem(STORE, JSON.stringify(servers));
    } catch {}
  }

  const json = (body) =>
    new Response(JSON.stringify(body), { headers: { "Content-Type": "application/json" } });
  const fail = (message, status = 400) => new Response(message, { status });

  const presetOf = (settings) => presets.find((p) => p.name === settings.preset);
  const gameTypeOf = (settings) =>
    presetOf(settings)?.gameTypes.find((t) => t.name === settings.gameType);

  // Why settings aren't valid, like the gateway's checks, or nothing when they are. Maps end up
  // in the game's config, so only the preset's are allowed.
  function invalid(settings) {
    const preset = presetOf(settings);
    if (!preset) return "unknown game";
    if (!gameTypeOf(settings)) return "unknown game type";
    const name = (settings.name ?? "").trim();
    if (!name || name.length > MAX_NAME) return `names are 1 to ${MAX_NAME} characters`;
    if (!settings.maps?.length || settings.maps.length > MAX_ROTATION) {
      return `choose 1 to ${MAX_ROTATION} maps`;
    }
    const unknown = settings.maps.find((map) => !preset.maps.includes(map));
    if (unknown) return `unknown map ${unknown}`;
    const limits = [
      ["bots", settings.bots, 0, 16],
      ["the bots' skill", settings.botSkill, 1, 5],
      ["the frag limit", settings.fragLimit, 0, 999],
      ["the time limit", settings.timeLimit, 0, 999],
      ["the capture limit", settings.captureLimit, 0, 999],
      ["max players", settings.maxPlayers, 0, 64],
    ];
    for (const [what, value, min, max] of limits) {
      if (value != null && !(Number.isInteger(value) && value >= min && value <= max)) {
        return `${what} must be between ${min} and ${max}`;
      }
    }
  }

  // Only what a local game uses, since the dialogs send every field they have.
  function clean(settings) {
    const { name, preset, gameType, maps, bots, botSkill, fragLimit, timeLimit, captureLimit, maxPlayers } = settings;
    return { name: name.trim(), preset, gameType, maps, bots, botSkill, fragLimit, timeLimit, captureLimit, maxPlayers };
  }

  function newId(name, servers) {
    const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "server";
    let id = slug;
    for (let n = 2; servers.some((server) => server.id === id); n++) id = `${slug}-${n}`;
    return id;
  }

  // The phase a server appears to be in, or nothing once it's running.
  function phaseOf({ startedAt, deletedAt }) {
    if (deletedAt) return "terminating";
    let left = Date.now() - (startedAt ?? 0);
    for (const [phase, duration] of PHASES) {
      if (left < duration) return phase;
      left -= duration;
    }
  }

  // What the gateway says about a server.
  function view(server) {
    const { id, settings } = server;
    const map = settings.maps[0];
    const phase = phaseOf(server);
    return {
      namespace: NAMESPACE,
      name: id,
      game: presetOf(settings)?.title ?? null,
      ready: !phase,
      phase,
      address: null,
      hostname: settings.name,
      map,
      mapTitle: presetOf(settings)?.mapInfo?.[map.toLowerCase()]?.title ?? null,
      rotation: settings.maps.length,
      gameType: settings.gameType,
      humans: 0,
      bots: settings.bots,
      maxPlayers: settings.maxPlayers || 8,
      needsPassword: false,
      message: null,
      owner: null,
      createdHere: true,
      mine: true,
      manageable: !server.deletedAt,
      settings: server.deletedAt ? null : settings,
      deletesAt: null,
    };
  }

  function api(method, path, options) {
    if (path === "/api/config") {
      return json({
        user: null,
        canCreate: true,
        nativeClients: false,
        maxServers: MAX_SERVERS,
        maxServersPerUser: null,
        idleTimeout: null,
        presets,
      });
    }
    const servers = load();
    if (path === "/api/servers") {
      if (method === "GET") return json(servers.map(view));
      if (servers.length >= MAX_SERVERS) {
        return fail(`The limit of ${MAX_SERVERS} servers has been reached.`, 409);
      }
      const settings = JSON.parse(options.body);
      const problem = invalid(settings);
      if (problem) return fail(problem);
      const server = { id: newId(settings.name, servers), settings: clean(settings), startedAt: Date.now() };
      save([...servers, server]);
      return json(view(server));
    }
    const [, id, status] = path.match(`^/api/servers/${NAMESPACE}/([^/]+)(/status)?$`) ?? [];
    const server = servers.find((s) => s.id === id);
    if (!server) return fail("server not found", 404);
    if (status) {
      const { settings } = server;
      return json({
        ready: !phaseOf(server),
        game: presetOf(settings)?.title ?? null,
        map: settings.maps[0],
        players: [],
        fragLimit: settings.fragLimit,
        timeLimit: settings.timeLimit,
        captureLimit: settings.captureLimit,
      });
    }
    if (method === "DELETE") {
      server.deletedAt ??= Date.now();
      save(servers);
      return new Response(null, { status: 204 });
    }
    if (server.deletedAt) return fail("The server is being deleted.", 409);
    const settings = JSON.parse(options.body);
    const problem = invalid(settings);
    if (problem) return fail(problem);
    // Switching games replaces the pod, on a real cluster.
    if (settings.preset !== server.settings.preset) server.startedAt = Date.now();
    server.settings = clean(settings);
    save(servers);
    return json({ applied: "restart" });
  }

  const contentUrl = (file) => {
    const remote = globalThis.previewRemote[file.split("/")[0]];
    return `${remote ?? `${root}/content`}/${file}`;
  };

  // Fetches ranges of a pak, one after the other. The gateway takes a list of them in a POST,
  // but static hosting only has Range requests.
  async function readRanges(pak, ranges) {
    const spans = [];
    for (const [start, end] of ranges) {
      const last = spans[spans.length - 1];
      if (last && start - last[1] < RANGE_GAP) last[1] = end;
      else spans.push([start, end]);
    }
    const parts = new Map();
    const pending = [...spans];
    const worker = async () => {
      for (let span; (span = pending.shift()); ) {
        const response = await realFetch(contentUrl(pak), {
          headers: { Range: `bytes=${span[0]}-${span[1] - 1}` },
        });
        // A compressed response's ranges are of the compressed file.
        if (response.status !== 206 || response.headers.has("Content-Encoding")) {
          throw new Error(`reading ${pak} failed (${response.status})`);
        }
        parts.set(span, new Uint8Array(await response.arrayBuffer()));
      }
    };
    await Promise.all(Array.from({ length: RANGE_CONCURRENCY }, worker));
    const data = new Uint8Array(ranges.reduce((sum, [start, end]) => sum + end - start, 0));
    let at = 0;
    for (const [start, end] of ranges) {
      const span = spans.find(([from, to]) => from <= start && end <= to);
      data.set(parts.get(span).subarray(start - span[0], end - span[0]), at);
      at += end - start;
    }
    return new Response(data);
  }

  // The requests the play page makes of its server.
  function game(method, id, path, options) {
    const server = load().find((s) => s.id === id);
    if (!server) return fail("server not found", 404);
    if (path === "info") {
      return json({ mapname: server.settings.maps[0], game: presetOf(server.settings)?.game ?? "" });
    }
    if (path === "assets/manifest.json") return realFetch(`${root}/manifest.json`);
    if (path === "paks/profile.json") return realFetch(`${root}/profile.json`);
    if (path.startsWith("paks/")) {
      if (method === "POST") return readRanges(path.slice("paks/".length), JSON.parse(options.body));
      return realFetch(`${root}/${path}`);
    }
    if (path.startsWith("assets/")) {
      // Asset names come with their checksums, for the gateway's sake.
      const pak = path.slice("assets/".length).replace(/(^|\/)\d+-([^/]+)$/, "$1$2");
      return realFetch(contentUrl(pak));
    }
    return fail("not found", 404);
  }

  const realFetch = window.fetch.bind(window);
  window.fetch = async (input, options = {}) => {
    const url = new URL(input instanceof Request ? input.url : input, location.href);
    if (url.origin === location.origin) {
      const method = (options.method ?? "GET").toUpperCase();
      if (url.pathname.startsWith("/api/servers") || url.pathname === "/api/config") {
        return api(method, url.pathname, options);
      }
      const match = url.pathname.match(`^/play/${NAMESPACE}/([^/]+)/(.+)$`);
      if (match) return game(method, match[1], match[2], options);
    }
    return realFetch(input, options);
  };

  // A local game has no one to talk to, so the browser is never asked for the microphone, however
  // the engine comes to want it. It carries on without one, as if permission was refused.
  const noMicrophone = () => {
    console.warn("Preview: refused a request for the microphone");
    return Promise.reject(new DOMException("A local game has no voice chat.", "NotAllowedError"));
  };
  if (navigator.mediaDevices) navigator.mediaDevices.getUserMedia = noMicrophone;
  for (const name of ["getUserMedia", "webkitGetUserMedia", "mozGetUserMedia"]) {
    if (navigator[name]) navigator[name] = noMicrophone;
  }

  // The game's config: the settings, and the rotation as the game server writes it (see Maps in
  // internal/quake/server/config.go).
  function renderConfig(settings) {
    const type = gameTypeOf(settings)?.type ?? 0;
    const preset = presetOf(settings);
    const cvars = { ...preset?.spec?.cvars, ...preset?.cvars, ...gameTypeOf(settings)?.cvars };
    const lines = [
      ...Object.entries(cvars).map(([name, value]) => `seta ${name} "${value}"`),
      // The name is the only free text, so it can't end the quotes or start another command.
      `seta sv_hostname "${settings.name.replace(/["\\;\r\n]/g, "")}"`,
      `seta bot_minplayers ${settings.bots || 0}`,
      // There's nobody else to balance teams against, so don't leave the player spectating.
      "seta g_teamAutoJoin 1",
    ];
    if (settings.botSkill) lines.push(`seta g_spSkill ${settings.botSkill}`);
    if (settings.maxPlayers) lines.push(`seta sv_maxclients ${settings.maxPlayers}`);
    settings.maps.forEach((map, i) => {
      const cmds = [`g_gametype ${type}`];
      if (settings.captureLimit) cmds.push(`capturelimit ${settings.captureLimit}`);
      if (settings.fragLimit) cmds.push(`fraglimit ${settings.fragLimit}`);
      if (settings.timeLimit) cmds.push(`timelimit ${settings.timeLimit}`);
      cmds.push(`map ${map}`);
      cmds.push(`set nextmap vstr d${(i + 1) % settings.maps.length}`);
      lines.push(`set d${i} "seta ${cmds.join(" ; ")}"`);
    });
    lines.push("vstr d0", ...(preset?.commands ?? []), "");
    return lines.join("\n");
  }

  const id = new URLSearchParams(location.search).get("server");
  const server = load().find((s) => s.id === id);
  if (location.pathname.endsWith("/game.html") && !server) location.replace(`${root}/`);

  const serverPreset = server && presetOf(server.settings);
  const serverGame = serverPreset?.game || serverPreset?.baseGame;

  globalThis.preview = {
    root,
    playUrl: (s) => `${root}/game.html?server=${encodeURIComponent(s.name)}`,
    // The server the play page is for.
    server: { name: id ?? "", title: server?.settings.name ?? "", maps: server?.settings.maps ?? [] },
    manage: () => (server ? { settings: server.settings, presets } : null),
    baseGame: serverPreset?.baseGame ?? "baseoa",
    // In place of connecting to a server.
    launchArgs: () => ["+set", "sv_pure", "0", "+set", "dedicated", "0", "+exec", CONFIG],
    configFile: () => ({
      path: `/${serverGame}/${CONFIG}`,
      data: new TextEncoder().encode(renderConfig(server.settings)),
    }),
    // A game's loose files, which a game server has on its disk.
    looseFiles: () =>
      Promise.all(
        globalThis.previewLoose
          .filter((file) => file.split("/")[0] === serverGame)
          .map(async (file) => {
            const response = await realFetch(contentUrl(file));
            if (!response.ok) throw new Error(`Downloading ${file} failed (${response.status}).`);
            return { path: `/${file}`, data: new Uint8Array(await response.arrayBuffer()) };
          }),
      ),
    // Takes a server for this tab, for as long as it's open, so a server can't run twice.
    // Reports whether this tab got it.
    claim: () =>
      new Promise((resolve) => {
        if (!navigator.locks) return resolve(true);
        navigator.locks.request(`server-${id}`, { ifAvailable: true }, (lock) => {
          resolve(Boolean(lock));
          return lock && new Promise(() => {});
        });
      }),
  };

  // The list refreshes every few seconds, which would skip over most of the phases. It refreshes
  // once more after the last, to show how it ended.
  if (!location.pathname.endsWith("/game.html")) {
    let changing = false;
    setInterval(() => {
      const changed = changing;
      changing = load().some(phaseOf);
      if (changing || changed) globalThis.refresh?.();
    }, 500);
  }

  document.addEventListener("DOMContentLoaded", () => {
    // There's nobody to keep out of a local game, and changes reload it.
    const style = document.createElement("style");
    style.textContent = "#s-note { display: none; } main p.hint a { color: var(--accent); }";
    document.head.append(style);
    const password = document.querySelector('#create-form input[name="password"]');
    if (password) {
      password.disabled = true;
      password.closest("label").hidden = true;
    }

    const join = document.querySelector("#join h1");
    if (join) {
      join.textContent = server?.settings.name ?? "";
      document.title = `${server?.settings.name ?? "Preview"} · Quake Kube`;
    }

    const hint = document.getElementById("hint");
    if (hint) {
      const banner = document.createElement("p");
      banner.className = "hint";
      banner.append(
        "This is a preview. Games run in your browser against bots, and servers are only saved in " +
          "this browser, with no Kubernetes involved. ",
      );
      const links = [
        ["Install the real thing", "../getting-started.html"],
        ["Credits and content removal", "../credits.html"],
      ];
      links.forEach(([text, href], i) => {
        const a = document.createElement("a");
        a.textContent = text;
        a.href = href;
        banner.append(...(i ? [" · ", a] : [a]));
      });
      document.querySelector("main header").after(banner);
    }
  });
}
