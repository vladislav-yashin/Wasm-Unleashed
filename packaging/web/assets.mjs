/** HTTP resource source. Consumers must await read before entering game states. */
export class AssetSource {
  constructor(base, manifest) {
    this.base = base;
    this.manifest = manifest;
    this.pending = new Map();
    this.cacheName = 'open-unleashed-assets-objects-v1';
  }

  static async open(base = './data/') {
    const url = new URL(base, location.href);
    const response = await fetch(new URL('manifest.json', url), { cache: 'no-cache' });
    if (!response.ok) throw new Error(`Manifest HTTP ${response.status}`);
    const manifest = await response.json();
    if (manifest.schema !== 1 || !manifest.files || !manifest.version)
      throw new Error('Unsupported asset manifest');
    return new AssetSource(url, manifest);
  }

  async read(path) {
    const key = path.replaceAll('\\', '/').toLowerCase();
    const entry = this.manifest.files[key];
    if (!entry) throw new Error(`Missing asset: ${path}`);
    if (!this.pending.has(key)) {
      this.pending.set(key, this.load(entry).finally(() => this.pending.delete(key)));
    }
    return (await this.pending.get(key)).slice(0);
  }

  async object(entry) {
    const key = `object:${entry.sha256}`;
    if (!this.pending.has(key)) this.pending.set(key, this.load(entry).finally(() => this.pending.delete(key)));
    return this.pending.get(key);
  }

  async load(entry) {
    if (entry.segments) {
      const bytes = new Uint8Array(entry.size);
      for (const segment of entry.segments) bytes.set(new Uint8Array(await this.object(segment)), segment.offset);
      if (!await this.valid(bytes.buffer, entry)) throw new Error(`Corrupt archive: ${entry.path}`);
      return bytes.buffer;
    }
    const url = new URL(entry.url, this.base);
    let cache;
    try { cache = await caches.open(this.cacheName); } catch { /* ephemeral mode */ }
    const cached = cache && await cache.match(url);
    if (cached) {
      const bytes = await cached.arrayBuffer();
      if (await this.valid(bytes, entry)) return bytes;
      await cache.delete(url);
    }
    const response = await fetch(url);
    if (!response.ok) throw new Error(`Asset HTTP ${response.status}: ${entry.path}`);
    const bytes = await response.arrayBuffer();
    if (!await this.valid(bytes, entry)) throw new Error(`Corrupt asset: ${entry.path}`);
    try { await cache?.put(url, new Response(bytes)); } catch { /* quota: keep playing */ }
    return bytes;
  }

  async valid(bytes, entry) {
    if (bytes.byteLength !== entry.size) return false;
    const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
    return Array.from(digest, value => value.toString(16).padStart(2, '0')).join('') === entry.sha256;
  }
}
