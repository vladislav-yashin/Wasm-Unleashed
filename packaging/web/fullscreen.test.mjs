import { test } from 'node:test';
import assert from 'node:assert/strict';
import { enterFullscreen, requestLaunchFullscreen, touchLayout } from './fullscreen.mjs';

function setup({coarse = true, supported = true, active = false, reject = false, lockRejects = false} = {}) {
  const calls = {fullscreen: 0, lock: []};
  const document = {
    fullscreenEnabled: supported,
    fullscreenElement: active ? {tagName: 'HTML'} : null,
    documentElement: {
      async requestFullscreen() {
        calls.fullscreen++;
        if (reject) throw new Error('denied');
      },
    },
  };
  const screen = {
    orientation: {
      async lock(mode) {
        calls.lock.push(mode);
        if (lockRejects) throw new Error('unsupported');
      },
    },
  };
  const media = query => { assert.equal(query, '(any-pointer:coarse)'); return {matches: coarse}; };
  return {environment: {document, screen, matchMedia: media}, calls};
}

test('touch layout follows the coarse pointer query', () => {
  assert.equal(touchLayout(() => ({matches: true})), true);
  assert.equal(touchLayout(() => ({matches: false})), false);
  assert.equal(touchLayout(undefined), false);
});

test('launch enters fullscreen and landscape once on touch devices', async () => {
  const {environment, calls} = setup();
  assert.equal(await requestLaunchFullscreen(environment), true);
  assert.equal(calls.fullscreen, 1);
  assert.deepEqual(calls.lock, ['landscape']);
  assert.equal(await requestLaunchFullscreen(environment), false);
  assert.equal(calls.fullscreen, 1);
});

test('desktop launch keeps the windowed page', async () => {
  const {requestLaunchFullscreen: launch} = await import('./fullscreen.mjs?desktop');
  const {environment, calls} = setup({coarse: false});
  assert.equal(await launch(environment), false);
  assert.equal(calls.fullscreen, 0);
  assert.deepEqual(calls.lock, []);
});

test('the toolbar button enters fullscreen on desktops without a landscape lock', async () => {
  const {environment, calls} = setup({coarse: false});
  assert.equal(await enterFullscreen(environment), true);
  assert.equal(calls.fullscreen, 1);
  assert.deepEqual(calls.lock, []);
});

test('an active fullscreen session is left alone', async () => {
  const {environment, calls} = setup({active: true});
  assert.equal(await enterFullscreen(environment), false);
  assert.equal(calls.fullscreen, 0);
});

test('a browser without fullscreen support is ignored', async () => {
  const {environment, calls} = setup({supported: false});
  assert.equal(await enterFullscreen(environment), false);
  assert.equal(calls.fullscreen, 0);
});

test('a rejected request stays silent', async () => {
  const {environment, calls} = setup({reject: true});
  assert.equal(await enterFullscreen(environment), false);
  assert.equal(calls.fullscreen, 1);
  assert.deepEqual(calls.lock, []);
});

test('a refused landscape lock does not fail fullscreen', async () => {
  const {environment} = setup({lockRejects: true});
  assert.equal(await enterFullscreen(environment), true);
});
