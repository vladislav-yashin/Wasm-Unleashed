import { test } from 'node:test';
import assert from 'node:assert/strict';

function setup({fullscreen = false, flushError = false, exitError = false} = {}) {
  const calls = [];
  const environment = {
    document: {
      documentElement: {dataset: {}},
      fullscreenElement: fullscreen ? {tagName: 'HTML'} : null,
      async exitFullscreen() {
        calls.push('fullscreen');
        if (exitError) throw new Error('denied');
      },
    },
    location: {
      replace(target) {
        calls.push(`replace:${target}`);
      },
    },
    nfsUpdateControls(mode) {
      calls.push(`controls:${mode}`);
    },
  };
  const flush = async () => {
    calls.push('flush');
    if (flushError) throw new Error('storage');
  };
  return {environment, calls, flush};
}

const fresh = name => import(`./exit-game.mjs?${name}`);

test('saves, releases the controls, leaves fullscreen and returns home', async () => {
  const {leaveGame} = await fresh('normal');
  const {environment, calls, flush} = setup({fullscreen: true});
  assert.equal(await leaveGame(environment, flush), true);
  assert.equal(environment.document.documentElement.dataset.gameMode, 'exited');
  assert.deepEqual(calls, ['controls:exited', 'flush', 'fullscreen', 'replace:./']);
});

test('a windowed exit does not touch fullscreen', async () => {
  const {leaveGame} = await fresh('windowed');
  const {environment, calls, flush} = setup();
  await leaveGame(environment, flush);
  assert.deepEqual(calls, ['controls:exited', 'flush', 'replace:./']);
});

test('a failed save still returns home', async () => {
  const {leaveGame} = await fresh('save-failed');
  const {environment, calls, flush} = setup({flushError: true});
  const logged = [];
  const original = console.error;
  console.error = (...args) => logged.push(args);
  try {
    assert.equal(await leaveGame(environment, flush), true);
  } finally {
    console.error = original;
  }
  assert.deepEqual(calls, ['controls:exited', 'flush', 'replace:./']);
  assert.equal(logged.length, 1);
});

test('a refused fullscreen leave still returns home', async () => {
  const {leaveGame} = await fresh('fullscreen-failed');
  const {environment, calls, flush} = setup({fullscreen: true, exitError: true});
  assert.equal(await leaveGame(environment, flush), true);
  assert.deepEqual(calls, ['controls:exited', 'flush', 'fullscreen', 'replace:./']);
});

test('a repeated exit leaves once', async () => {
  const {leaveGame} = await fresh('repeated');
  const {environment, calls, flush} = setup();
  assert.equal(await leaveGame(environment, flush), true);
  assert.equal(await leaveGame(environment, flush), false);
  assert.deepEqual(calls, ['controls:exited', 'flush', 'replace:./']);
});
