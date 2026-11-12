const keyOf = path => {
  const parts = String(path).replaceAll('\\', '/').split('/').filter(p => p && p !== '.');
  if (parts.includes('..')) throw new Error('Parent traversal is not allowed');
  return parts.join('/').toLowerCase();
};

/** Read-only installed assets and writable /saves, backed by IndexedDB. */
export class VirtualFileSystem {
  constructor(source) {
    this.source = source;
    this.files = new Map();
    this.sparse = new Map();
    this.directories = new Set(['', 'game', 'saves']);
    this.known = new Set();
    this.dirty = new Map();
    this.revision = 0;
    this.flushing = null;
    for (const path of Object.keys(source.manifest.files)) {
      const key = `game/${keyOf(path)}`;
      this.known.add(key);
      this.addParents(key);
    }
  }
  addParents(key) {
    const parts = key.split('/');
    parts.pop();
    while (parts.length) { this.directories.add(parts.join('/')); parts.pop(); }
  }
  stat(path) {
    const key = keyOf(path);
    return this.files.has(key) || this.known.has(key) ? 1 : this.directories.has(key) ? 2 : 0;
  }
  read(path) {
    const key = keyOf(path);
    if (!this.files.has(key)) throw new Error(`Resource not preloaded: ${path}`);
    return this.files.get(key).slice();
  }
  length(path) {
    const key = keyOf(path);
    if (this.sparse.has(key)) return this.sparse.get(key).size;
    const bytes = this.files.get(key);
    if (!bytes) throw new Error(`Resource not preloaded: ${path}`);
    return bytes.length;
  }
  readRange(path, start, length) {
    const sparse = this.sparse.get(keyOf(path));
    if (sparse) {
      const end = Math.min(start + length, sparse.size);
      if (start >= sparse.size) return new Uint8Array();
      for (const [offset, bytes] of sparse.ranges) {
        if (offset <= start && offset + bytes.length >= end) return bytes.slice(start-offset, end-offset);
      }
      throw new Error(`Archive range not preloaded: ${path} at ${start}`);
    }
    const bytes = this.files.get(keyOf(path));
    if (!bytes) throw new Error(`Resource not preloaded: ${path}`);
    return bytes.slice(start, start + length);
  }
  async readRangeAsync(path, start, length) {
    const sparse = this.sparse.get(keyOf(path));
    if (!sparse) return this.readRange(path, start, length);
    if (start < 0 || length < 0 || start + length > sparse.size) throw new Error('Invalid archive range');
    const result = new Uint8Array(length);
    for (const segment of sparse.segments) {
      if (segment.offset >= start + length || segment.offset + segment.size <= start) continue;
      let bytes = sparse.ranges.get(segment.offset);
      if (!bytes || bytes.length < segment.size) {
        bytes = new Uint8Array(await this.source.object(segment));
        sparse.ranges.set(segment.offset, bytes);
      }
      const begin = Math.max(start, segment.offset), end = Math.min(start + length, segment.offset + segment.size);
      result.set(bytes.subarray(begin-segment.offset,end-segment.offset),begin-start);
    }
    return result;
  }
  list(path) {
    const key = keyOf(path);
    if (!this.directories.has(key)) throw new Error(`Directory not found: ${path}`);
    const prefix = key ? `${key}/` : '';
    const names = new Set();
    for (const item of [...this.known, ...this.files.keys(), ...this.directories]) {
      if (item.startsWith(prefix)) {
        const rest = item.slice(prefix.length);
        if (rest) names.add(rest.split('/')[0]);
      }
    }
    return [...names].sort();
  }
  writable(key) {
    if (!key.startsWith('saves/')) throw new Error(`Read-only path: ${key}`);
  }
  write(path, bytes) {
    const key = keyOf(path);
    this.writable(key);
    this.addParents(key);
    const data = new Uint8Array(bytes).slice();
    this.files.set(key, data);
    this.dirty.set(key, { revision: ++this.revision, data });
  }
  mkdir(path) {
    const key = keyOf(path);
    if (key !== 'saves') this.writable(key);
    this.directories.add(key);
    this.addParents(key);
  }
  remove(path) {
    const key = keyOf(path);
    this.writable(key);
    if (!this.files.delete(key)) throw new Error(`File not found: ${path}`);
    this.dirty.set(key, { revision: ++this.revision, data: null });
  }
  async preload(paths, progress = () => {}, concurrency = 4) {
    const queue = [...new Set(paths.map(keyOf))];
    let index = 0, done = 0;
    const worker = async () => {
      while (index < queue.length) {
        const path = queue[index++];
        const key = `game/${path}`;
        const entry = this.source.manifest.files[path];
        if (entry?.segments && !this.sparse.has(key)) {
          const ranges = new Map();
          for (const segment of entry.segments) ranges.set(segment.offset, new Uint8Array(await this.source.object(segment.head)));
          this.sparse.set(key, {size:entry.size, segments:entry.segments, ranges});
        } else if (!entry?.segments && !this.files.has(key)) this.files.set(key, new Uint8Array(await this.source.read(path)));
        progress(++done, queue.length, path);
      }
    };
    await Promise.all(Array.from({ length: Math.min(concurrency, queue.length) }, worker));
  }
  unload(paths) {
    for (const path of paths) { this.files.delete(`game/${keyOf(path)}`); this.sparse.delete(`game/${keyOf(path)}`); }
  }
  async openSaves() {
    this.db = await new Promise((resolve, reject) => {
      const request = indexedDB.open('open-unleashed', 1);
      request.onupgradeneeded = () => request.result.createObjectStore('files');
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
      request.onblocked = () => reject(new Error('Save database upgrade blocked by another tab'));
    });
    const rows = await new Promise((resolve, reject) => {
      const tx = this.db.transaction('files', 'readonly');
      const rows = [];
      const request = tx.objectStore('files').openCursor();
      request.onsuccess = () => {
        const cursor = request.result;
        if (cursor) { rows.push([cursor.key, cursor.value]); cursor.continue(); }
      };
      tx.oncomplete = () => resolve(rows);
      tx.onabort = () => reject(tx.error);
    });
    for (const [path, bytes] of rows) {
      const key = keyOf(path);
      this.writable(key);
      this.files.set(key, new Uint8Array(bytes));
      this.addParents(key);
    }
  }
  async flush() {
    if (this.flushing) { await this.flushing; return this.flush(); }
    if (!this.dirty.size) return;
    if (!this.db) throw new Error('Persistent saves unavailable');
    const snapshot = new Map(this.dirty);
    this.flushing = new Promise((resolve, reject) => {
      const tx = this.db.transaction('files', 'readwrite');
      const store = tx.objectStore('files');
      for (const [key, entry] of snapshot) {
        if (entry.data === null) store.delete(key); else store.put(entry.data, key);
      }
      tx.oncomplete = resolve;
      tx.onabort = () => reject(tx.error || new Error('Save transaction aborted'));
    });
    try {
      await this.flushing;
      for (const [key, entry] of snapshot) {
        if (this.dirty.get(key)?.revision === entry.revision) this.dirty.delete(key);
      }
    } finally { this.flushing = null; }
  }
  exportSaves() {
    return JSON.stringify({ schema: 1, files: Object.fromEntries(
      [...this.files].filter(([key]) => key.startsWith('saves/')).map(([key, data]) => [key, Array.from(data)])) });
  }
  async importSaves(text) {
    const archive = JSON.parse(text);
    if (archive.schema !== 1 || !archive.files || typeof archive.files !== 'object') throw new Error('Invalid save archive');
    const entries = Object.entries(archive.files);
    for (const [path, bytes] of entries) {
      this.writable(keyOf(path));
      if (!Array.isArray(bytes) || !bytes.every(v => Number.isInteger(v) && v >= 0 && v <= 255)) throw new Error(`Invalid save: ${path}`);
    }
    for (const [path, bytes] of entries) this.write(path, bytes);
    await this.flush();
  }
}
