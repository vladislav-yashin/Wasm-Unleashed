import { test } from 'node:test';
import assert from 'node:assert/strict';
import { AssetSource } from './assets.mjs';
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
