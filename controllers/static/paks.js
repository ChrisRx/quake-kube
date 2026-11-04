// Paks the web client reads in parts, fetching from the game server only what the engine
// reads, and keeping what's fetched in the browser's cache for next time. The client built with JSPI calls open, size, read and fetch from its pak reads (see
// controllers/client/patches/0002-web-wait-for-pak-reads-with-jspi.patch), and waits while
// fetch runs. Load after defining `base`, the server's path on the gateway.

class RemotePaks {
  // How much more to fetch after a file the engine reads, since files next to each other in a
  // pak are often read together.
  static READ_AHEAD = 512 * 1024;

  // Prefetched files are fetched in batches of up to this size, this many at once.
  static BATCH = 8 * 1024 * 1024;
  static CONCURRENCY = 6;

  // Fetched ranges are cached by their pak's checksum, so a changed pak never reads old parts.
  static CACHE = "quake-kube-ranges-2";
  // Older caches, which are removed. The first held ranges GitHub Pages sent of its compressed
  // copies of paks, with nothing to tell them apart, under the same names as the real ones.
  static OLD_CACHES = ["quake-kube-ranges"];

  constructor() {
    this.paks = [];
    this.ids = new Map();
    this.bytes = 0;
    this.requests = 0;
    this.cachedBytes = 0;
    this.cache = null;
  }

  // Adds a pak from the manifest, with its index from the game server.
  async add(asset) {
    const response = await fetch(`${base}/paks/${asset.name}.json`);
    if (!response.ok) {
      throw new Error(`Indexing ${asset.name} failed (${response.status}).`);
    }
    const index = await response.json();
    this.paks.push({
      name: asset.name,
      checksum: asset.checksum,
      index,
      // The pak's files' ranges in order, to find the files around a position.
      files: Object.values(index.files).sort((a, b) => a[0] - b[0]),
      // Fetched ranges in order, which never overlap: { start, end, data }.
      chunks: [],
      // Entries in the browser's cache, which are read when needed: { key, ranges }.
      cached: [],
    });
    this.ids.set(asset.name, this.paks.length);
  }

  // Finds which ranges of the paks are in the browser's cache, without reading them, and
  // removes those of paks that have since changed. The cache needs HTTPS.
  async openCache() {
    if (globalThis.caches) {
      await Promise.all(RemotePaks.OLD_CACHES.map((name) => caches.delete(name).catch(() => {})));
    }
    this.cache = globalThis.caches
      ? await caches.open(RemotePaks.CACHE).catch(() => null)
      : null;
    if (!this.cache) return;
    for (const request of await this.cache.keys()) {
      const match = new URL(request.url, "https://x").pathname.match(/^\/ranges\/(\d+)\/(.+)\/([\d,-]+)$/);
      const pak = match && this.paks[this.ids.get(decodeURIComponent(match[2])) - 1];
      if (!pak) continue;
      if (String(pak.checksum) !== match[1]) {
        this.cache.delete(request).catch(() => {});
        continue;
      }
      const ranges = match[3].split(",").map((range) => range.split("-").map(Number));
      pak.cached.push({ key: request.url, ranges });
    }
  }

  // Each request's ranges are cached as one entry, named by its pak and ranges.
  cacheKey(pak, ranges) {
    return `/ranges/${pak.checksum}/${pak.name}/${ranges.map((range) => range.join("-")).join(",")}`;
  }

  // The pak at a path the engine opens, e.g. "./q3ut4/zUrT43_004.pk3", or 0 for other files.
  open(path) {
    return this.ids.get(path.replace(/^\.?\/+/, "")) ?? 0;
  }

  size(id) {
    return this.paks[id - 1].index.size;
  }

  // Copies a range into dest if it has been fetched, returning how much was copied, which is
  // less than asked for at the end of the pak, or -1 when part of it is missing.
  read(id, pos, dest) {
    const pak = this.paks[id - 1];
    const end = Math.min(pos + dest.length, pak.index.size);
    if (pos >= end) return 0;
    let at = pos;
    for (let i = this.chunkAt(pak, at); i < pak.chunks.length && at < end; i++) {
      const chunk = pak.chunks[i];
      if (chunk.start > at) return -1;
      const n = Math.min(chunk.end, end) - at;
      dest.set(chunk.data.subarray(at - chunk.start, at - chunk.start + n), at - pos);
      at += n;
    }
    return at >= end ? end - pos : -1;
  }

  // Fetches what the engine is reading, and what follows it, before the engine reads it again.
  async fetch(id, pos, size) {
    const pak = this.paks[id - 1];
    const end = Math.min(pos + size, pak.index.size);
    const [dirStart, dirEnd] = pak.index.directory;
    let ranges;
    if (pos >= dirStart && end <= dirEnd) {
      ranges = [[dirStart, dirEnd]];
    } else {
      ranges = [];
      let budget = RemotePaks.READ_AHEAD;
      for (let i = this.fileAt(pak, pos); i < pak.files.length; i++) {
        const [start, stop] = pak.files[i];
        if (start >= end && budget <= 0) break;
        if (!this.covered(pak, start, stop)) {
          ranges.push([start, stop]);
          if (start >= end) budget -= stop - start;
        }
      }
      // Anywhere else, e.g. between files, only what's read.
      if (!ranges.some(([start, stop]) => start <= pos && pos < stop)) {
        ranges.unshift([pos, end]);
      }
    }
    try {
      await this.fetchRanges(pak, ranges);
    } catch (error) {
      // The engine treats the missing part as the end of the pak, and fails to read the file.
      console.warn(`Fetching from ${pak.name} failed:`, error);
    }
  }

  // Fetches files before the engine reads them, e.g. those a profile lists, by pak name:
  // { "q3ut4/zUrT43_008.pk3": ["scripts/urban_terror.shader"] }. Calls onProgress with the
  // size of each batch fetched, and returns how much there is to fetch.
  prefetch(files, onProgress = () => {}) {
    const batches = [];
    for (const [name, names] of Object.entries(files)) {
      const id = this.ids.get(name);
      if (!id) continue;
      const pak = this.paks[id - 1];
      const ranges = names
        .map((file) => pak.index.files[file])
        .filter((range) => range && !this.covered(pak, range[0], range[1]))
        .sort((a, b) => a[0] - b[0]);
      let batch = [];
      let size = 0;
      for (const [start, end] of ranges) {
        if (size > 0 && size + end - start > RemotePaks.BATCH) {
          batches.push({ pak, ranges: batch, size });
          batch = [];
          size = 0;
        }
        // Files next to each other become one range.
        const last = batch[batch.length - 1];
        if (last && last[1] === start) last[1] = end;
        else if (!last || last[1] < start) batch.push([start, end]);
        else continue;
        size += end - start;
      }
      if (size > 0) batches.push({ pak, ranges: batch, size });
    }
    const total = batches.reduce((sum, batch) => sum + batch.size, 0);
    const worker = async () => {
      for (let batch; (batch = batches.shift()); ) {
        try {
          await this.fetchRanges(batch.pak, batch.ranges);
        } catch (error) {
          // Whatever's missing is fetched when the engine reads it.
          console.warn(`Prefetching from ${batch.pak.name} failed:`, error);
        }
        onProgress(batch.size);
      }
    };
    const done = Promise.all(Array.from({ length: RemotePaks.CONCURRENCY }, worker));
    return { total, done };
  }

  // Reads ranges from the browser's cache, fetching the rest from the game server and caching
  // them.
  async fetchRanges(pak, ranges) {
    const missing = await this.readCache(pak, ranges);
    if (missing.length === 0) return;
    this.requests++;
    const response = await fetch(`${base}/paks/${pak.name}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(missing),
    });
    if (!response.ok) {
      throw new Error(`${response.status} ${await response.text()}`);
    }
    const data = new Uint8Array(await response.arrayBuffer());
    this.insertAll(pak, missing, data);
    this.bytes += data.length;
    if (this.cache) {
      const key = this.cacheKey(pak, missing);
      pak.cached.push({ key, ranges: missing });
      this.cache.put(key, new Response(data)).catch(() => {});
    }
  }

  // Adds ranges fetched together, whose data is one after the other.
  insertAll(pak, ranges, data) {
    let offset = 0;
    for (const [start, end] of ranges) {
      this.insert(pak, start, end, data.subarray(offset, offset + end - start));
      offset += end - start;
    }
  }

  // Reads the cache entries with ranges overlapping the ones wanted, returning those still
  // missing.
  async readCache(pak, ranges) {
    const overlaps = ([start, end]) =>
      ranges.some(([wantStart, wantEnd]) => start < wantEnd && wantStart < end);
    const hits = pak.cached.filter(
      (entry) => !entry.read && entry.ranges.some((range) => overlaps(range) && !this.covered(pak, ...range)),
    );
    await Promise.all(
      hits.map(async (entry) => {
        entry.read = true;
        const response = await this.cache.match(entry.key).catch(() => null);
        if (!response) return;
        const data = new Uint8Array(await response.arrayBuffer());
        if (data.length !== entry.ranges.reduce((sum, [start, end]) => sum + end - start, 0)) return;
        this.insertAll(pak, entry.ranges, data);
        this.cachedBytes += data.length;
      }),
    );
    return ranges.filter(([start, end]) => !this.covered(pak, start, end));
  }

  // The index of the last chunk starting at or before pos, or 0.
  chunkAt(pak, pos) {
    let lo = 0;
    let hi = pak.chunks.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (pak.chunks[mid].start <= pos) lo = mid;
      else hi = mid - 1;
    }
    return Math.max(lo, 0);
  }

  // The index of the first file ending after pos.
  fileAt(pak, pos) {
    let lo = 0;
    let hi = pak.files.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (pak.files[mid][1] <= pos) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }

  // Whether a range has been fetched, in one chunk or several next to each other.
  covered(pak, start, end) {
    let at = start;
    for (let i = this.chunkAt(pak, at); i < pak.chunks.length && at < end; i++) {
      if (pak.chunks[i].start > at) return false;
      at = Math.max(at, pak.chunks[i].end);
    }
    return at >= end;
  }

  // Adds a fetched range, leaving out any parts already fetched so chunks never overlap.
  // Chunks next to each other aren't merged, since read spans them, and merging would copy
  // ever larger chunks as a pak is read in order.
  insert(pak, start, end, data) {
    let i = this.chunkAt(pak, start);
    if (pak.chunks[i] && pak.chunks[i].end <= start) i++;
    let at = start;
    const gaps = [];
    for (; i < pak.chunks.length && pak.chunks[i].start < end; i++) {
      if (pak.chunks[i].start > at) gaps.push([at, pak.chunks[i].start]);
      at = Math.max(at, pak.chunks[i].end);
    }
    if (at < end) gaps.push([at, end]);
    for (const [gapStart, gapEnd] of gaps) {
      let j = this.chunkAt(pak, gapStart);
      if (pak.chunks[j] && pak.chunks[j].start < gapStart) j++;
      pak.chunks.splice(j, 0, {
        start: gapStart,
        end: gapEnd,
        data: data.slice(gapStart - start, gapEnd - start),
      });
    }
  }

  stats() {
    return { bytes: this.bytes, requests: this.requests, cachedBytes: this.cachedBytes };
  }
}
