import { test } from 'node:test';
import assert from 'node:assert/strict';
import { VirtualFileSystem } from './vfs.mjs';
const source = () => ({ manifest: { files: { 'fedata/menu.bin': {} } }, read: async () => new Uint8Array([1, 2]).buffer });
test('manifest directories work before loading; reads require preload', async () => {
  const fs = new VirtualFileSystem(source());
  assert.equal(fs.stat('/GAME/FEData/menu.bin'), 1);
  assert.deepEqual(fs.list('game'), ['fedata']);
  assert.throws(() => fs.read('game/fedata/menu.bin'), /not preloaded/);
  await fs.preload(['FEData/menu.bin']);
  const bytes = fs.read('game/fedata/menu.bin');
  bytes[0] = 9;
  assert.deepEqual(fs.read('game/fedata/menu.bin'), new Uint8Array([1, 2]));
  fs.unload(['fedata/menu.bin']);
  assert.equal(fs.stat('game/fedata/menu.bin'), 1);
});
test('assets immutable; saves isolated and exported; unavailable persistence reported', async () => {
  const fs = new VirtualFileSystem(source());
  assert.throws(() => fs.write('game/fedata/menu.bin', [9]), /Read-only/);
  assert.throws(() => fs.write('saves/../game/a', [9]), /traversal/);
  fs.write('/saves/A.sav', [4, 5]);
  assert.deepEqual(fs.list('saves'), ['a.sav']);
  assert.deepEqual(JSON.parse(fs.exportSaves()).files['saves/a.sav'], [4, 5]);
  await assert.rejects(fs.flush(), /unavailable/);
  assert.equal(fs.dirty.size, 1);
  fs.remove('saves/a.sav');
  assert.equal(fs.stat('saves/a.sav'), 0);
});
test('preload concurrency bounded and failures retryable', async () => {
  let running = 0, peak = 0, fail = true;
  const fs = new VirtualFileSystem({manifest: {files: {}}, read: async path => {
    running++; peak = Math.max(peak, running);
    await new Promise(resolve => setTimeout(resolve, 2)); running--;
    if (path === 'bad' && fail) throw new Error('network');
    return new Uint8Array([1]).buffer;
  }});
  await fs.preload(['a', 'b', 'c'], () => {}, 2);
  assert.equal(peak, 2);
  await assert.rejects(fs.preload(['bad']), /network/);
  fail = false;
  await fs.preload(['bad']);
  assert.equal(fs.read('game/bad')[0], 1);
});
test('music archive preloads headers and fetches only the requested song', async () => {
  const requests=[];
  const segments=[{offset:0,size:4,id:'directory',head:{id:'directory'}},{offset:4,size:8,id:'song',head:{id:'song-head'}},{offset:12,size:8,id:'other',head:{id:'other-head'}}];
  const data={'directory':[1,2,3,4],'song-head':[5,6],'other-head':[13,14],song:[5,6,7,8,9,10,11,12],other:[13,14,15,16,17,18,19,20]};
  const fs=new VirtualFileSystem({manifest:{files:{'gamedata/music/zzzymus.viv':{size:20,segments}}},object:async entry=>{requests.push(entry.id);return new Uint8Array(data[entry.id]).buffer;}});
  await fs.preload(['gamedata/music/zzzymus.viv']);
  assert.deepEqual(requests,['directory','song-head','other-head']);
  assert.equal(fs.length('/game/gamedata/music/zzzymus.viv'),20);
  assert.deepEqual(fs.readRange('game/gamedata/music/zzzymus.viv',4,2),new Uint8Array([5,6]));
  assert.throws(()=>fs.readRange('game/gamedata/music/zzzymus.viv',4,8),/not preloaded/);
  assert.deepEqual(await fs.readRangeAsync('game/gamedata/music/zzzymus.viv',5,3),new Uint8Array([6,7,8]));
  assert.deepEqual(requests,['directory','song-head','other-head','song']);
});
