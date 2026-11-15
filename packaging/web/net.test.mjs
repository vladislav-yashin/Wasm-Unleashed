import assert from 'node:assert/strict';
import test from 'node:test';
import { CLOSED, CONNECTING, OPEN, RelayClient, resolveRelayUrl } from './net.mjs';

test('the relay URL is same-origin by default', () => {
  assert.equal(resolveRelayUrl({ protocol: 'http:', host: '127.0.0.1:8080' }).toString(), 'ws://127.0.0.1:8080/relay');
  assert.equal(resolveRelayUrl({ protocol: 'https:', host: 'example.test' }).toString(), 'wss://example.test/relay');
});

test('?relay= overrides the default and inherits ws(s)', () => {
  const url = resolveRelayUrl({ protocol: 'http:', host: 'example.test', search: '?relay=relay.test:9000/udp' });
  assert.equal(url.toString(), 'ws://relay.test:9000/udp');
  assert.equal(resolveRelayUrl({ protocol: 'https:', host: 'example.test', override: 'wss://relay.test/relay' }).toString(), 'wss://relay.test/relay');
  assert.throws(() => resolveRelayUrl({ protocol: 'https:', host: 'example.test', override: 'ws://relay.test/relay' }), /wss/);
  assert.equal(resolveRelayUrl({ protocol: 'http:', host: 'example.test', override: 'https://relay.test/relay' }).toString(), 'wss://relay.test/relay');
  assert.throws(() => resolveRelayUrl({ protocol: 'http:', host: 'example.test', override: 'ftp://relay.test/relay' }), /ws/);
});

class FakeSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;
  constructor(url) {
    this.url = url;
    this.readyState = FakeSocket.CONNECTING;
    this.binaryType = 'blob';
    this.sent = [];
    this.closed = false;
    FakeSocket.last = this;
  }
  send(bytes) { this.sent.push(Uint8Array.from(bytes)); }
  close() { this.closed = true; this.readyState = FakeSocket.CLOSED; this.onclose?.(); }
  open() { this.readyState = FakeSocket.OPEN; this.onopen?.(); }
  message(bytes) { this.onmessage?.({ data: Uint8Array.from(bytes).buffer }); }
}

const client = () => new RelayClient({
  location: { protocol: 'http:', host: '127.0.0.1:8080', search: '' },
  socketFactory: url => new FakeSocket(url),
});

test('open names the target and queues sends until the socket opens', () => {
  const c = client();
  const id = c.open('192.168.1.5:9442');
  assert.equal(id, 1);
  const socket = FakeSocket.last;
  assert.equal(socket.url, 'ws://127.0.0.1:8080/relay?target=192.168.1.5%3A9442');
  assert.equal(c.status(), CONNECTING);
  assert.equal(c.send(Uint8Array.from([1, 2, 3])), true);
  assert.equal(socket.sent.length, 0);
  socket.open();
  assert.equal(c.status(), OPEN);
  assert.deepEqual(socket.sent, [Uint8Array.from([1, 2, 3])]);
  assert.equal(c.send(Uint8Array.from([4])), true);
  assert.deepEqual(socket.sent, [Uint8Array.from([1, 2, 3]), Uint8Array.from([4])]);
});

test('received binary frames are handed over once', () => {
  const c = client();
  c.open('1.2.3.4:9442');
  FakeSocket.last.open();
  FakeSocket.last.message([9, 8]);
  FakeSocket.last.onmessage?.({ data: 'text is ignored' });
  assert.deepEqual(c.recv(), [Uint8Array.from([9, 8])]);
  assert.deepEqual(c.recv(), []);
});

test('only the matching generation can close a connection', () => {
  const c = client();
  const first = c.open('1.2.3.4:9442');
  const firstSocket = FakeSocket.last;
  const second = c.open('5.6.7.8:9442');
  const secondSocket = FakeSocket.last;
  assert.equal(firstSocket.closed, true);
  c.close(first);
  assert.equal(secondSocket.closed, false);
  assert.equal(c.status(), CONNECTING);
  c.close(second);
  assert.equal(secondSocket.closed, true);
  assert.equal(c.status(), -1);
});

test('a failed handshake reports closed and refuses new sends', () => {
  const c = client();
  c.open('1.2.3.4:9442');
  const socket = FakeSocket.last;
  socket.onerror?.(new Error('refused'));
  assert.equal(c.status(), CLOSED);
  assert.equal(c.send(Uint8Array.from([1])), false);
});
