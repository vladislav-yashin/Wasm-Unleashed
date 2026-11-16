import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {AssetSource} from './assets.mjs';
import {applyMods, discoverMods} from './mods.mjs';
import {VirtualFileSystem} from './vfs.mjs';

const base = new URL('https://example.test/data/');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const object = bytes => ({url: `objects/${hash(bytes)}`, sha256: hash(bytes), size: bytes.length});
function pack(id, files, enabled = false) {
  const bytes = JSON.stringify({schema: 1, version: id, files});
  return {bytes, entry: {id, name: id, description: 'A mod', enabled_by_default: enabled,
    sha256: hash(bytes), size: Buffer.byteLength(bytes), manifest: `mods/${id}/${hash(bytes)}.json`}};
}

test('catalog is optional, validates defaults, and reports server or malformed catalog errors', async () => {
  globalThis.fetch = async () => new Response('', {status:404});
  assert.deepEqual(await discoverMods(base), []);
  globalThis.fetch = async () => new Response('', {status:503});
  await assert.rejects(discoverMods(base), /503/);
  const {entry} = pack('cars', {}, true);
  globalThis.fetch = async () => Response.json({schema:1, packs:[entry]});
  assert.equal((await discoverMods(base))[0].enabled_by_default, true);
  globalThis.fetch = async () => Response.json({schema:1, packs:[entry, entry]});
  await assert.rejects(discoverMods(base), /Invalid/);
  globalThis.fetch = async () => Response.json({schema:1, packs:[{...entry, enabled_by_default:'false'}]});
  await assert.rejects(discoverMods(base), /Invalid/);
});

test('selected overlays replace case-insensitive VFS resources in order and retain segmented music', async () => {
  const original = object('original'), first = object('first'), last = object('last');
  const song = object('song');
  const music = {...song, path:'GameData/Music/zzzymus.viv', segments:[{...song, offset:0, head:song}]};
  delete music.url;
  const a = pack('first', {'fedata/menu.bin':first});
  const b = pack('last', {'fedata/menu.bin':last, 'gamedata/music/zzzymus.viv':music});
  const source = new AssetSource(base, {schema:1, version:'base', files:{'fedata/menu.bin':original, 'fe.txt':original}});
  const objects = new Map([[a.entry.manifest,a.bytes], [b.entry.manifest,b.bytes], [last.url,'last'], [song.url,'song']]);
  globalThis.fetch = async url => new Response(objects.get(String(url).replace(base.href,'')));
  const merged = await applyMods(source, [a.entry,b.entry]);
  assert.equal(merged.manifest.files['fe.txt'], original);
  assert.equal(source.manifest.files['fedata/menu.bin'], original);
  const fs = new VirtualFileSystem(merged);
  await fs.preload(['fedata/menu.bin']);
  assert.equal(new TextDecoder().decode(fs.read('/game/FEData/MENU.bin')), 'last');
  assert.equal(new TextDecoder().decode(await merged.read('GameData/Music/zzzymus.viv')), 'song');
  assert.equal((await applyMods(source, [])).manifest.files['fedata/menu.bin'], original);
});

test('missing or corrupt selected manifests fail without changing original resources', async () => {
  const source = new AssetSource(base, {version:'base',files:{}});
  const {entry} = pack('bad', {});
  globalThis.fetch = async () => new Response('', {status:404});
  await assert.rejects(applyMods(source,[entry]), /404/);
  globalThis.fetch = async () => new Response('corrupt');
  await assert.rejects(applyMods(source,[entry]), /Corrupt/);
  assert.deepEqual(source.manifest.files, {});
});
