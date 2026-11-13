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
    const digest = globalThis.crypto?.subtle
      ? new Uint8Array(await globalThis.crypto.subtle.digest('SHA-256', bytes))
      : await sha256(bytes);
    return Array.from(digest, value => value.toString(16).padStart(2, '0')).join('') === entry.sha256;
  }
}

// Web Crypto is unavailable on plain HTTP LAN origins. Keep content verification
// there as well; yield between batches so large archives do not freeze the UI.
const SHA256_K = new Uint32Array([
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);
const rotate = (x, n) => (x >>> n) | (x << (32 - n));

export async function sha256(buffer) {
  const input = new Uint8Array(buffer);
  const state = new Uint32Array([0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19]);
  const words = new Uint32Array(64);
  const tail = new Uint8Array(input.length % 64 < 56 ? 64 : 128);
  tail.set(input.subarray(input.length - input.length % 64));
  tail[input.length % 64] = 0x80;
  const length = new DataView(tail.buffer);
  length.setUint32(tail.length - 8, Math.floor(input.length / 0x20000000));
  length.setUint32(tail.length - 4, (input.length * 8) >>> 0);
  const complete = input.length - input.length % 64;
  const view = new DataView(input.buffer, input.byteOffset, input.byteLength);
  for (let offset = 0; offset < complete + tail.length; offset += 64) {
    const block = offset < complete ? view : length;
    const start = offset < complete ? offset : offset - complete;
    for (let i = 0; i < 16; i++) words[i] = block.getUint32(start + i * 4);
    for (let i = 16; i < 64; i++) {
      const x = words[i - 15], y = words[i - 2];
      words[i] = words[i - 16] + (rotate(x, 7) ^ rotate(x, 18) ^ (x >>> 3)) + words[i - 7] + (rotate(y, 17) ^ rotate(y, 19) ^ (y >>> 10));
    }
    let [a, b, c, d, e, f, g, h] = state;
    for (let i = 0; i < 64; i++) {
      const t1 = (h + (rotate(e, 6) ^ rotate(e, 11) ^ rotate(e, 25)) + ((e & f) ^ (~e & g)) + SHA256_K[i] + words[i]) >>> 0;
      const t2 = ((rotate(a, 2) ^ rotate(a, 13) ^ rotate(a, 22)) + ((a & b) ^ (a & c) ^ (b & c))) >>> 0;
      h = g; g = f; f = e; e = (d + t1) >>> 0; d = c; c = b; b = a; a = (t1 + t2) >>> 0;
    }
    const next = [a, b, c, d, e, f, g, h];
    for (let i = 0; i < 8; i++) state[i] += next[i];
    if ((offset + 64) % (256 * 1024) === 0) await new Promise(resolve => setTimeout(resolve, 0));
  }
  const result = new Uint8Array(32);
  const output = new DataView(result.buffer);
  for (let i = 0; i < 8; i++) output.setUint32(i * 4, state[i]);
  return result;
}
