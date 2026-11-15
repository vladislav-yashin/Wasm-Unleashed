/**
 * The browser's WebSocket bridge to the UDP relay.
 *
 * The game (WASM) calls the `globalThis.nfsNet*` functions installed by `installNetwork()`. They are
 * deliberately small and synchronous: `open` starts the handshake, `send` queues while it is in
 * flight, and `recv` hands the game the frames that arrived since the last call. The game's frame
 * format lives in Rust (`nfs_net::relay`); this module treats frames as opaque bytes.
 *
 * One WebSocket is bound to one destination (`?target=ip:port`). The relay URL comes from
 * `globalThis.nfsNetRelay`, then `?relay=`, and otherwise the page's own origin at `/relay`.
 */

export const CONNECTING = 0;
export const OPEN = 1;
export const CLOSED = 2;

/** Resolve the relay URL, including `?relay=` and the HTTPS rule. */
export function resolveRelayUrl({ protocol, host, search = '', override } = {}) {
  const query = search ? new URLSearchParams(search) : null;
  const chosen = override || query?.get('relay') || '';
  const origin = `${protocol}//${host}`;
  const scheme = protocol === 'https:' ? 'wss:' : 'ws:';
  // `?relay=relay.test:8090/relay` is a host and path, not a URL scheme.
  const base = chosen ? (/^[a-z][a-z0-9+.-]*:\/\//i.test(chosen) ? chosen : `${scheme}//${chosen}`) : `${scheme}//${host}/relay`;
  let url;
  try {
    url = new URL(base, origin);
  } catch {
    throw new Error(`Invalid relay URL: ${base}`);
  }
  if (url.protocol === 'http:') url.protocol = 'ws:';
  if (url.protocol === 'https:') url.protocol = 'wss:';
  if (url.protocol !== 'ws:' && url.protocol !== 'wss:') {
    throw new Error(`The relay URL must be ws:// or wss://, not ${url.protocol}`);
  }
  if (protocol === 'https:' && url.protocol === 'ws:') {
    throw new Error('An HTTPS page needs a wss:// relay URL');
  }
  return url;
}

/** One relay connection with a bounded send queue and an inbox for the game. */
export class RelayClient {
  constructor({ location = globalThis.location, search = location?.search || '', relay = globalThis.nfsNetRelay, socketFactory } = {}) {
    this.location = location;
    this.search = search;
    this.relay = relay;
    this.socketFactory = socketFactory || (url => new WebSocket(url));
    this.nextId = 1;
    this.state = null;
  }

  /** Start a connection to `target` (`ip:port`). Returns its generation id, or 0 on a bad URL. */
  open(target) {
    let url;
    try {
      url = resolveRelayUrl({
        protocol: this.location?.protocol || 'http:',
        host: this.location?.host || '127.0.0.1',
        search: this.search,
        override: this.relay,
      });
    } catch (error) {
      console.error(error);
      return 0;
    }
    url.searchParams.set('target', target);
    this.close();
    const id = this.nextId++;
    const state = { id, socket: null, status: CONNECTING, inbox: [], outbox: [] };
    this.state = state;
    let socket;
    try {
      socket = this.socketFactory(url.toString());
    } catch (error) {
      console.error(error);
      state.status = CLOSED;
      return id;
    }
    state.socket = socket;
    socket.binaryType = 'arraybuffer';
    socket.onopen = () => {
      if (this.state !== state) return;
      state.status = OPEN;
      for (const bytes of state.outbox.splice(0)) socket.send(bytes);
    };
    socket.onmessage = event => {
      if (this.state !== state) return;
      const data = event?.data;
      if (data instanceof ArrayBuffer) state.inbox.push(new Uint8Array(data));
      else if (ArrayBuffer.isView(data)) state.inbox.push(new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
    };
    const failed = () => {
      if (this.state === state) state.status = CLOSED;
    };
    socket.onerror = failed;
    socket.onclose = failed;
    return id;
  }

  /** Queue a frame until the socket opens, or send it now. `false` means no connection at all. */
  send(bytes) {
    const state = this.state;
    if (!state) return false;
    // The caller's `Uint8Array` is a view into the WASM heap; copy it before keeping it past the
    // call, because the heap can be reused while the WebSocket handshake is still in flight.
    const frame = Uint8Array.from(bytes);
    if (state.status === OPEN && state.socket?.readyState === 1) {
      state.socket.send(frame);
      return true;
    }
    if (state.status === CONNECTING) {
      if (state.outbox.length < 64) state.outbox.push(frame);
      return true;
    }
    return false;
  }

  /** Take the frames received since the last call. */
  recv() {
    return this.state ? this.state.inbox.splice(0) : [];
  }

  status() {
    return this.state ? this.state.status : -1;
  }

  /** Close only the connection with this generation, so an idle lobby cannot close a live game. */
  close(id) {
    const state = this.state;
    if (!state || (id !== undefined && state.id !== id)) return;
    this.state = null;
    try {
      if (state.socket && state.socket.readyState < 2) state.socket.close();
    } catch {
      /* already gone */
    }
  }
}

/** Install the functions `nfs_platform::net` calls. Safe to call more than once. */
export function installNetwork(global = globalThis, options = {}) {
  if (global.nfsNetOpen) return;
  const client = new RelayClient(options);
  global.nfsNetOpen = target => client.open(target);
  global.nfsNetSend = bytes => client.send(bytes);
  global.nfsNetRecv = () => client.recv();
  global.nfsNetStatus = () => client.status();
  global.nfsNetClose = id => client.close(id);
}
