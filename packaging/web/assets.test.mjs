import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { AssetSource, sha256 } from './assets.mjs';
const bytes = new Uint8Array([1,2,3]);
const hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), x => x.toString(16).padStart(2,'0')).join('');
const entry = {path:'FEData/menu',url:'objects/hash',size:3,sha256:hash};
const source = () => new AssetSource(new URL('https://example.test/data/'), {version:'test',files:{'fedata/menu':entry}});
test('concurrent callers share request but own returned bytes; ephemeral cache supported', async () => {
  let requests = 0;
  globalThis.fetch = async () => { requests++; await new Promise(r=>setTimeout(r,1)); return new Response(bytes); };
  const assets = source();
  const [a,b] = await Promise.all([assets.read('FEData/menu'),assets.read('fedata/menu')]);
  assert.equal(requests,1);
  new Uint8Array(a)[0] = 99;
  assert.equal(new Uint8Array(b)[0],1);
});
test('bad hash and HTTP failures reject and allow retry', async () => {
  const assets=source();
  globalThis.fetch=async()=>new Response(new Uint8Array([9,9,9]));
  await assert.rejects(assets.read('fedata/menu'), /Corrupt/);
  globalThis.fetch=async()=>new Response('',{status:404});
  await assert.rejects(assets.read('fedata/menu'), /404/);
  globalThis.fetch=async()=>new Response(bytes);
  assert.equal((await assets.read('fedata/menu')).byteLength,3);
  await assert.rejects(assets.read('missing'), /Missing/);
});

test('HTTP SHA-256 fallback matches standard vectors, padding boundaries and large data', async () => {
  for (const size of [0, 1, 3, 55, 56, 63, 64, 65, 119, 120, 127, 128, 1000000]) {
    const data = Uint8Array.from({length: size}, (_, i) => i % 251);
    const actual = Buffer.from(await sha256(data.buffer)).toString('hex');
    assert.equal(actual, createHash('sha256').update(data).digest('hex'), `length ${size}`);
  }
  assert.equal(Buffer.from(await sha256(new TextEncoder().encode('abc').buffer)).toString('hex'),
    'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
});

test('plain HTTP without SubtleCrypto still accepts valid assets and rejects corruption', async () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'crypto');
  try {
    for (const crypto of [{}, undefined]) {
      Object.defineProperty(globalThis, 'crypto', { configurable: true, value: crypto });
      const assets = source();
      globalThis.fetch = async () => new Response(bytes);
      assert.deepEqual(new Uint8Array(await assets.read('FEData/menu')), bytes);
      globalThis.fetch = async () => new Response(new Uint8Array([9, 9, 9]));
      await assert.rejects(assets.read('fedata/menu'), /Corrupt asset/);
      assert.equal(await assets.valid(bytes.buffer, {...entry, size: 4}), false);
    }
  } finally {
    if (descriptor) Object.defineProperty(globalThis, 'crypto', descriptor);
    else delete globalThis.crypto;
  }
});

test('fallback yields to the event loop while hashing large archives', async () => {
  let yielded = false;
  const timer = setTimeout(() => { yielded = true; }, 0);
  try {
    await sha256(new ArrayBuffer(1024 * 1024));
    assert.equal(yielded, true);
  } finally { clearTimeout(timer); }
});
