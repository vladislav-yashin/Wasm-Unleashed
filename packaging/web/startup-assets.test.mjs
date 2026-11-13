import { test } from 'node:test';
import assert from 'node:assert/strict';
import { carPaths, gamePaths, initialPaths } from './startup-assets.mjs';
import { VirtualFileSystem } from './vfs.mjs';

const manifestFiles = paths => Object.fromEntries(paths.map(path => [path, {}]));

test('startup keeps intro, entry movie and timeline metadata, deferring other media', () => {
  const keep = ['fe.txt', 'madtables.bin', 'fedata/movies/earts-av.mad',
    'fedata/movies/inpc-av.mad', 'fedata/movies/ch6.mad', 'fedata/chron/icons356.fsh',
    'fedata/layouts/chronicle.lay', 'fedata/locale/festrings.loc'];
  const deferred = ['fedata/movies/ch1.mad', 'fedata/movies/er1pc-av.mad',
    'fedata/chron/pa481a.fsh', 'fedata/showcase/356-14.fsh'];
  assert.deepEqual(initialPaths(manifestFiles([...keep, ...deferred])), keep);
});

test('startup retains Alps and shared track data; other tracks remain deferred', () => {
  const keep = ['gamedata/track/alps.crp', 'gamedata/track/alps_st1.scn',
    'gamedata/track/animdefs.txt', 'gamedata/track/default.bin', 'gamedata/track/data/skies.bin'];
  assert.deepEqual(initialPaths(manifestFiles([...keep, 'gamedata/track/coastal.crp'])), keep);
});

test('model selection uses complete families without overlapping name prefixes', () => {
  const files = manifestFiles(['993.crp', '993.tpg', '993d.fsh', '993l.fsh',
    '993.clr', '993gt.crp', '993gt.tpg', 'eas3.crp', 'eas3.tpg', 'eas3l.fsh',
    'shadow.fsh', 'packages.ini'].map(name => `gamedata/carmodel/${name}`));
  assert.deepEqual(carPaths(files, ['993', '993']), ['993.crp', '993.tpg', '993d.fsh', '993l.fsh']
    .map(name => `gamedata/carmodel/${name}`));
  assert.deepEqual(carPaths(files, ['EAS3']), ['eas3.crp', 'eas3.tpg', 'eas3l.fsh']
    .map(name => `gamedata/carmodel/${name}`));
  assert.deepEqual(initialPaths(files), ['993.crp', '993.tpg', '993d.fsh', '993l.fsh',
    '993.clr', 'shadow.fsh', 'packages.ini'].map(name => `gamedata/carmodel/${name}`));
});

test('deferred media stays discoverable and is fetched only when selected; failures can retry', async () => {
  const files = manifestFiles(['fe.txt', 'fedata/movies/ch1.mad', 'fedata/movies/ch2.mad',
    'fedata/chron/pa481a.fsh', 'fedata/showcase/356-14.fsh']);
  const fetched = [];
  let fail = true;
  const fs = new VirtualFileSystem({ manifest: { files }, read: async path => {
    fetched.push(path);
    if (fail && path === 'fedata/movies/ch1.mad') throw new Error('offline');
    return new Uint8Array([1, 2, 3]).buffer;
  }});
  await fs.preload(initialPaths(files));
  assert.deepEqual(fetched, ['fe.txt']);
  assert.equal(fs.stat('/game/fedata/movies/ch1.mad'), 1);
  assert.throws(() => fs.read('/game/fedata/movies/ch1.mad'), /not preloaded/);
  const selected = gamePaths(['/GAME/FEData/Movies/Ch1.mad', 'game\\fedata\\movies\\ch1.mad']);
  await assert.rejects(fs.preload(selected), /offline/);
  fail = false;
  await fs.preload(selected);
  assert.deepEqual(fs.read('/game/fedata/movies/ch1.mad'), new Uint8Array([1, 2, 3]));
  await fs.preload(selected);
  assert.deepEqual(fetched, ['fe.txt', 'fedata/movies/ch1.mad', 'fedata/movies/ch1.mad']);
  assert.equal(fs.files.has('game/fedata/movies/ch2.mad'), false);
  for (const path of ['/saves/profile.sav', '/game/../saves/profile.sav']) {
    assert.throws(() => gamePaths([path]), /Invalid game resource/);
  }
});
