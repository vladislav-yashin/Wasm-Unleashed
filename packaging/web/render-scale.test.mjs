import { test } from 'node:test';
import assert from 'node:assert/strict';
import { applyScale, normalizeScale, SCALES, STORAGE_KEY, storedScale } from './render-scale.mjs';

/** A `localStorage` stand-in over a `Map`. */
const storage = entries => {
  const data = new Map(entries);
  return { data, getItem: key => data.get(key) ?? null, setItem: (key, value) => data.set(key, value) };
};

test('only the shipped scales are accepted, full resolution otherwise', () => {
  assert.deepEqual(SCALES, [1, 0.75, 0.5, 0.4]);
  for (const scale of SCALES) assert.equal(normalizeScale(scale), scale);
  for (const value of ['0.9', 2, 0, -1, 'x', '', null, undefined]) assert.equal(normalizeScale(value), 1, String(value));
});

test('the saved scale comes from storage, full resolution when unavailable', () => {
  assert.equal(storedScale(storage([[STORAGE_KEY, '0.5']])), 0.5);
  assert.equal(storedScale(storage([])), 1);
  assert.equal(storedScale(storage([[STORAGE_KEY, '0.9']])), 1);
  assert.equal(storedScale({ getItem() { throw new Error('denied'); } }), 1);
  assert.equal(storedScale(null), 1);
});

test('applying a scale publishes it to the game and remembers it', () => {
  const saved = storage([]);
  const target = {};
  assert.equal(applyScale('0.75', {storage: saved, target}), 0.75);
  assert.equal(target.nfsRenderScale, 0.75);
  assert.equal(saved.data.get(STORAGE_KEY), '0.75');
  assert.equal(applyScale('9', {storage: saved, target}), 1, 'an unknown value is full resolution');
  assert.equal(target.nfsRenderScale, 1);
});

test('a storage failure does not stop the scale from applying', () => {
  const target = {};
  const denied = { setItem() { throw new Error('quota'); } };
  assert.equal(applyScale(0.5, {storage: denied, target}), 0.5);
  assert.equal(target.nfsRenderScale, 0.5);
});
