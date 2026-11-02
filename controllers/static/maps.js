// A picker for a server's map rotation, shared by the web app's dialogs and the game's server
// settings. Maps are picked in the order they're played, with thumbnails, search, a filter for
// the maps made for the chosen game type, the preset's lists of maps, and lists people save
// in their browser.
"use strict";

{
  // Button rules carry an ID's weight, so the game's pause menu, which styles its buttons by
  // ID, doesn't override them.
  const style = document.createElement("style");
  style.textContent = `
    .map-picker {
      display: grid; gap: 10px; min-width: 0;
      font: 14px system-ui, sans-serif; letter-spacing: normal; text-transform: none; color: var(--text, #e8e2d9);
    }
    .map-picker .mp-bar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
    .map-picker .mp-bar input[type="search"], .map-picker .mp-bar select, .map-picker .mp-save input {
      min-width: 0; padding: 7px 9px; font: 14px system-ui, sans-serif;
      color: var(--text, #e8e2d9); background: var(--bg, #1b1916);
      border: 1px solid var(--line, #3a332c); border-radius: 0;
    }
    .map-picker .mp-bar input[type="search"] { flex: 1 1 160px; }
    .map-picker .mp-bar select { flex: 0 1 180px; }
    .map-picker .mp-toggle {
      display: flex; gap: 6px; align-items: center; font: 12px system-ui, sans-serif;
      letter-spacing: 0; text-transform: none; color: var(--muted, #a39a8f); white-space: nowrap;
    }
    .map-picker .mp-toggle input { width: auto; margin: 0; }
    :is(#pause, html) .map-picker button.mp-link {
      padding: 0; font: 12px ui-monospace, monospace; letter-spacing: 0.04em; text-transform: none;
      color: var(--muted, #a39a8f); background: none; border: 0; cursor: pointer; text-decoration: underline;
    }
    :is(#pause, html) .map-picker button.mp-link:hover { color: var(--text, #e8e2d9); background: none; }
    .map-picker .mp-pending { margin: 0; font: 12px ui-monospace, monospace; color: var(--muted, #a39a8f); }
    .map-picker .mp-status {
      display: flex; flex-wrap: wrap; gap: 6px 14px; align-items: baseline;
      font: 12px ui-monospace, monospace; color: var(--muted, #a39a8f);
    }
    .map-picker .mp-status b { font-weight: 600; color: var(--text, #e8e2d9); }
    .map-picker .mp-save { display: flex; gap: 8px; }
    .map-picker .mp-save input { flex: 1; }
    .map-picker .mp-grid {
      display: grid; grid-template-columns: repeat(auto-fill, minmax(112px, 1fr)); grid-auto-rows: max-content;
      gap: 8px; max-height: 300px; overflow-y: auto; padding: 1px;
    }
    :is(#pause, html) .map-picker button.mp-tile {
      position: relative; display: grid; gap: 0; padding: 0; overflow: hidden; text-align: left;
      font: 12px/1.3 system-ui, sans-serif; letter-spacing: 0; text-transform: none;
      color: var(--text, #e8e2d9); background: var(--bg, #1b1916); border: 1px solid var(--line, #3a332c); cursor: pointer;
    }
    :is(#pause, html) .map-picker button.mp-tile:hover { border-color: var(--muted, #a39a8f); background: var(--bg, #1b1916); }
    :is(#pause, html) .map-picker button.mp-tile[aria-pressed="true"] { border-color: var(--accent, #e0562b); outline: 1px solid var(--accent, #e0562b); }
    .map-picker .mp-shot { aspect-ratio: 4 / 3; background: #26221e center / cover no-repeat; }
    .map-picker .mp-shot.missing { background: repeating-linear-gradient(135deg, #221f1b 0 8px, #1b1916 8px 16px); }
    .map-picker .mp-name { padding: 5px 6px 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .map-picker .mp-file {
      padding: 0 6px 5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
      font: 10px ui-monospace, monospace; color: var(--muted, #a39a8f);
    }
    .map-picker .mp-order {
      position: absolute; top: 4px; left: 4px; min-width: 20px; padding: 2px 5px; text-align: center;
      font: 600 11px ui-monospace, monospace; color: #fff; background: var(--accent, #e0562b);
    }
    .map-picker .mp-empty { grid-column: 1 / -1; font: 13px system-ui, sans-serif; color: var(--muted, #a39a8f); }
  `;
  document.head.append(style);

  const STORAGE_KEY = "quake-kube-map-lists";

  // Lists people save, by preset, in their browser.
  function savedLists() {
    try {
      return JSON.parse(localStorage.getItem(STORAGE_KEY)) || {};
    } catch {
      return {};
    }
  }
  function storeLists(lists) {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(lists));
    } catch {}
  }

  const el = (tag, props = {}, ...children) => {
    const e = Object.assign(document.createElement(tag), props);
    e.append(...children);
    return e;
  };

  class MapPicker {
    // levelshot(preset, map) returns a map's thumbnail URL.
    constructor(container, { levelshot }) {
      this.levelshot = levelshot;
      this.preset = null;
      this.selected = [];
      this.arena = null;
      this.gameTypeName = "";
      this.onchange = null;

      this.search = el("input", { type: "search", placeholder: "Search maps" });
      // Checked by default too, since the picker can be in a form that's reset.
      this.filterBox = el("input", { type: "checkbox", checked: true, defaultChecked: true });
      this.filterLabel = el("label", { className: "mp-toggle" }, this.filterBox, "Made for this mode");
      this.lists = el("select", { ariaLabel: "Lists of maps" });
      const bar = el("div", { className: "mp-bar" }, this.search, this.lists, this.filterLabel);

      this.count = el("span");
      this.shown = el("span");
      const link = (text, fn) => {
        const b = el("button", { type: "button", className: "mp-link", textContent: text });
        b.addEventListener("click", fn);
        return b;
      };
      this.deleteLink = link("Delete list", () => this.deleteList());
      this.status = el(
        "div",
        { className: "mp-status" },
        this.count,
        link("Shuffle", () => this.shuffle()),
        link("Clear", () => this.set([])),
        link("Save as list", () => this.showSave(true)),
        this.deleteLink,
        this.shown,
      );

      this.saveName = el("input", { placeholder: "Name this list", maxLength: 40 });
      const saveButton = el("button", { type: "button", className: "mp-link", textContent: "Save" });
      const cancel = el("button", { type: "button", className: "mp-link", textContent: "Cancel" });
      this.saveRow = el("div", { className: "mp-save", hidden: true }, this.saveName, saveButton, cancel);
      saveButton.addEventListener("click", () => this.saveList());
      cancel.addEventListener("click", () => this.showSave(false));
      this.saveName.addEventListener("keydown", (e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          this.saveList();
        }
      });

      this.pending = el("p", {
        className: "mp-pending",
        hidden: true,
        textContent: "Map pictures and names are still being made. They'll show up here in a minute or two.",
      });

      this.grid = el("div", { className: "mp-grid", role: "group", ariaLabel: "Maps" });
      container.append(el("div", { className: "map-picker" }, bar, this.status, this.saveRow, this.pending, this.grid));

      this.search.addEventListener("input", () => this.render());
      this.filterBox.addEventListener("change", () => this.render());
      this.lists.addEventListener("change", () => this.applyList());
    }

    get value() {
      return [...this.selected];
    }

    // Shows a preset's maps, with maps picked and the game type chosen.
    setPreset(preset, { maps = [], gameType = "" } = {}) {
      this.preset = preset;
      this.tiles = new Map();
      this.search.value = "";
      this.showSave(false);
      this.pending.hidden = preset.levelshots !== false;
      this.grid.replaceChildren(
        ...preset.maps.map((name) => {
          const info = { ...preset.mapInfo?.[name.toLowerCase()] };
          // Some arena files just repeat the map's name as its title.
          if (info.title?.toLowerCase() === name.toLowerCase()) delete info.title;
          const shot = el("div", { className: "mp-shot" });
          const img = new Image();
          const url = this.levelshot(preset.name, name);
          img.onload = () => (shot.style.backgroundImage = `url("${url}")`);
          img.onerror = () => shot.classList.add("missing");
          img.src = url;
          const order = el("span", { className: "mp-order", hidden: true });
          const tile = el(
            "button",
            { type: "button", className: "mp-tile", title: info.title ? `${info.title} (${name})` : name },
            shot,
            el("span", { className: "mp-name", textContent: info.title || name }),
            el("span", { className: "mp-file", textContent: info.title ? name : "" }),
            order,
          );
          tile.addEventListener("click", () => this.toggle(name));
          this.tiles.set(name, { tile, order, info });
          return tile;
        }),
      );
      this.setGameType(gameType, false);
      this.set(maps.filter((m) => preset.maps.includes(m)));
      this.renderLists();
    }

    // Takes in a newer copy of the shown preset, once its map pictures and names are ready,
    // keeping what's picked.
    update(preset) {
      if (this.preset?.name !== preset.name || this.preset.levelshots || !preset.levelshots) return;
      const query = this.search.value;
      this.setPreset(preset, { maps: this.selected, gameType: this.gameTypeName });
      this.search.value = query;
      this.render();
    }

    // Offers the maps made for a game type first, by the type its maps' arena files name.
    setGameType(name, render = true) {
      this.gameTypeName = name;
      this.arena = this.preset?.gameTypes.find((t) => t.name === name)?.arena ?? null;
      this.filterLabel.hidden = !this.arena;
      if (render) this.render();
    }

    set(maps) {
      this.selected = [...maps];
      this.render();
      this.onchange?.(this.value);
    }

    toggle(name) {
      const i = this.selected.indexOf(name);
      this.set(i >= 0 ? this.selected.filter((m) => m !== name) : [...this.selected, name]);
    }

    shuffle() {
      const maps = [...this.selected];
      for (let i = maps.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [maps[i], maps[j]] = [maps[j], maps[i]];
      }
      this.set(maps);
    }

    madeFor(name) {
      const types = this.tiles.get(name).info.types;
      // Maps without an arena file are offered for anything.
      return !this.arena || !types?.length || types.includes(this.arena);
    }

    render() {
      if (!this.preset) return;
      const query = this.search.value.trim().toLowerCase();
      const filter = this.arena && this.filterBox.checked;
      let shown = 0;
      for (const [name, { tile, order, info }] of this.tiles) {
        const i = this.selected.indexOf(name);
        tile.setAttribute("aria-pressed", String(i >= 0));
        order.hidden = i < 0;
        order.textContent = i + 1;
        const matches = !query || name.toLowerCase().includes(query) || info.title?.toLowerCase().includes(query);
        // Picked maps stay in view.
        const visible = i >= 0 || (matches && (!filter || this.madeFor(name)));
        tile.hidden = !visible;
        if (visible) shown++;
      }
      const n = this.selected.length;
      this.count.replaceChildren(
        n === 0 ? "No maps picked" : el("b", { textContent: `${n} ${n === 1 ? "map" : "maps"}` }),
        n > 1 ? " in rotation" : "",
      );
      this.shown.textContent =
        shown < this.tiles.size
          ? `Showing ${shown} of ${this.tiles.size}${filter && !query ? ` made for ${this.gameTypeName}` : ""}`
          : "";
      let empty = this.grid.querySelector(".mp-empty");
      if (shown === 0) {
        empty ??= this.grid.appendChild(el("p", { className: "mp-empty" }));
        empty.textContent = query ? "No maps match." : "No maps are made for this mode. Turn off the filter to see them all.";
      } else {
        empty?.remove();
      }
    }

    // The preset's lists, and the ones saved in this browser.
    renderLists() {
      const preset = this.preset;
      const options = [new Option("Lists of maps", "")];
      const suggested = preset.mapLists ?? [];
      const saved = savedLists()[preset.name] ?? [];
      if (suggested.length) {
        const group = el("optgroup", { label: "Suggested" });
        group.append(...suggested.map((l, i) => new Option(l.name, `s${i}`)));
        options.push(group);
      }
      if (saved.length) {
        const group = el("optgroup", { label: "Saved" });
        group.append(...saved.map((l, i) => new Option(l.name, `u${i}`)));
        options.push(group);
      }
      this.lists.replaceChildren(...options);
      this.lists.hidden = suggested.length + saved.length === 0;
      this.deleteLink.hidden = true;
    }

    list(value) {
      const index = Number(value.slice(1));
      return value[0] === "s" ? this.preset.mapLists?.[index] : savedLists()[this.preset.name]?.[index];
    }

    applyList() {
      const value = this.lists.value;
      this.deleteLink.hidden = !value.startsWith("u");
      const list = value && this.list(value);
      // Only the maps the preset has, e.g. without the ones that need content images.
      if (list) this.set(list.maps.filter((m) => this.preset.maps.includes(m)));
    }

    showSave(show) {
      this.saveRow.hidden = !show;
      if (show) {
        this.saveName.value = "";
        this.saveName.focus();
      }
    }

    saveList() {
      const name = this.saveName.value.trim();
      if (!name || this.selected.length === 0) return;
      const lists = savedLists();
      const mine = (lists[this.preset.name] ??= []).filter((l) => l.name !== name);
      mine.push({ name, maps: this.value });
      lists[this.preset.name] = mine;
      storeLists(lists);
      this.showSave(false);
      this.renderLists();
      this.lists.value = `u${mine.length - 1}`;
      this.deleteLink.hidden = false;
    }

    deleteList() {
      const value = this.lists.value;
      if (!value.startsWith("u")) return;
      const lists = savedLists();
      lists[this.preset.name]?.splice(Number(value.slice(1)), 1);
      storeLists(lists);
      this.renderLists();
    }
  }

  globalThis.MapPicker = MapPicker;
}
