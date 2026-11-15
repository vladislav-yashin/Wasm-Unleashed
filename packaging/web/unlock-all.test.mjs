import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { applyUnlockName, DEFAULT_NAME, MAX_LENGTH, normalizeUnlockName, STORAGE_KEY, storedUnlockName } from './unlock-all.mjs';

/** A `localStorage` stand-in over a `Map`. */
const storage = entries => {
  const data = new Map(entries);
  return { data, getItem: key => data.get(key) ?? null, setItem: (key, value) => data.set(key, value) };
};

test('the profile-box rules apply to the launch name', () => {
  assert.equal(normalizeUnlockName(DEFAULT_NAME), DEFAULT_NAME);
  assert.equal(normalizeUnlockName('  Nick  '), 'Nick');
  assert.equal(normalizeUnlockName('Ätna'), 'Ätna', 'Latin-1 is what a .sav holds');
  assert.equal(normalizeUnlockName('A'.repeat(MAX_LENGTH)), 'A'.repeat(MAX_LENGTH));
  for (const value of ['', '   ', '\n', null, undefined, 'A'.repeat(MAX_LENGTH + 1), 'Ник']) {
    assert.equal(normalizeUnlockName(value), DEFAULT_NAME, JSON.stringify(value));
  }
  for (const char of ['%', '\\', '/', ':', '*', '?', '"', '<', '>', '|', '\u0000', '\u00ff']) {
    assert.equal(normalizeUnlockName(`a${char}b`), DEFAULT_NAME, JSON.stringify(char));
  }
});

test('the saved name comes from storage, the default when unavailable', () => {
  assert.equal(storedUnlockName(storage([[STORAGE_KEY, 'Nick']])), 'Nick');
  assert.equal(storedUnlockName(storage([])), DEFAULT_NAME);
  assert.equal(storedUnlockName(storage([[STORAGE_KEY, 'a/b']])), DEFAULT_NAME);
  assert.equal(storedUnlockName({ getItem() { throw new Error('denied'); } }), DEFAULT_NAME);
  assert.equal(storedUnlockName(null), DEFAULT_NAME);
});

test('applying a name publishes it to the game and remembers it', () => {
  const saved = storage([]);
  const target = {};
  assert.equal(applyUnlockName(' Nick ', { storage: saved, target }), 'Nick');
  assert.equal(target.nfsUnlockName, 'Nick');
  assert.equal(saved.data.get(STORAGE_KEY), 'Nick');
  assert.equal(applyUnlockName('a/b', { storage: saved, target }), DEFAULT_NAME);
  assert.equal(target.nfsUnlockName, DEFAULT_NAME);
});

test('a storage failure does not stop the name from applying', () => {
  const target = {};
  const denied = { setItem() { throw new Error('quota'); } };
  assert.equal(applyUnlockName('Nick', { storage: denied, target }), 'Nick');
  assert.equal(target.nfsUnlockName, 'Nick');
});

test('game.html offers the launch field', () => {
  const html = readFileSync(new URL('./game.html', import.meta.url), 'utf8');
  assert.match(html, /id="unlock-name"/);
  assert.match(html, /id="unlock-name"[^>]*maxlength="16"/);
});
