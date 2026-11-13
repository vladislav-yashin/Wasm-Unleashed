import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

function setup() {
  const keys = [];
  const listeners = new Map();
  const node = () => ({
    dataset: {}, style: {}, hidden: false, value: '', attributes: {},
    classList: { add() {}, remove() {} },
    addEventListener(type, fn) { this[type] = fn; },
    setAttribute(name, value) { this.attributes[name] = value; },
    contains(target) { return target === this; },
    setPointerCapture() {},
    focus() { document.activeElement = this; },
    blur() { document.activeElement = null; },
  });
  const canvas = { focus() {}, dispatchEvent(event) { keys.push([event.code, event.type]); } };
  const text = node(), toggle = node(), tools = node();
  const buttons = ['ArrowLeft', 'ArrowRight', 'ArrowUp'].map(code => Object.assign(node(), { dataset: { code } }));
  const group = { contains: button => buttons.slice(0, 2).includes(button) };
  for (const button of buttons) button.closest = selector => selector === '[data-code]' ? button : group;
  let under;
  const document = {
    documentElement: { dataset: {} }, activeElement: null,
    querySelector: selector => ({ canvas, '#text-input': text, '#tools-toggle': toggle, '#tools': tools })[selector],
    querySelectorAll: () => buttons,
    elementFromPoint: () => under,
    addEventListener: (type, fn) => listeners.set(`document:${type}`, fn),
  };
  const context = { document, innerWidth: 1000, innerHeight: 500,
    window: { addEventListener: (type, fn) => listeners.set(type, fn) },
    KeyboardEvent: class { constructor(type, values) { Object.assign(this, { type }, values); } },
  };
  runInNewContext(readFileSync(new URL('./controls.mjs', import.meta.url), 'utf8'), context);
  const event = pointerId => ({ pointerId, buttons: 1, preventDefault() {}, stopPropagation() {} });
  return { context, text, toggle, tools, buttons, keys, listeners, event, setUnder: button => { under = button; } };
}

test('steering slides between arrows while a second finger holds throttle', () => {
  const { buttons: [left, right, gas], keys, event, setUnder } = setup();
  left.onpointerdown(event(1)); gas.onpointerdown(event(2));
  setUnder(right); left.onpointermove(event(1));
  left.onpointerup(event(1)); gas.onpointercancel(event(2));
  assert.deepEqual(keys, [['ArrowLeft','keydown'], ['ArrowUp','keydown'], ['ArrowLeft','keyup'], ['ArrowRight','keydown'], ['ArrowRight','keyup'], ['ArrowUp','keyup']]);
});

test('a captured finger can leave a group and return; losing capture releases it', () => {
  const { buttons: [left, right], keys, event, setUnder } = setup();
  left.onpointerdown(event(1));
  setUnder(null); left.onpointermove(event(1));
  setUnder(right); left.onpointermove(event(1));
  left.onlostpointercapture(event(1));
  assert.deepEqual(keys, [['ArrowLeft','keydown'], ['ArrowLeft','keyup'], ['ArrowRight','keydown'], ['ArrowRight','keyup']]);
});

test('two fingers on one key release it only after both lift', () => {
  const { buttons: [left], keys, event } = setup();
  left.onpointerdown(event(1)); left.onpointerdown(event(2));
  left.onpointerup(event(1));
  assert.deepEqual(keys, [['ArrowLeft','keydown']]);
  left.onpointerup(event(2));
  assert.deepEqual(keys, [['ArrowLeft','keydown'], ['ArrowLeft','keyup']]);
});

test('pause, tab hiding and blur release held controls', () => {
  for (const reason of ['pause', 'dialog', 'hidden', 'blur', 'resize']) {
    const { context, buttons: [left], keys, event, listeners } = setup();
    left.onpointerdown(event(1));
    if (reason === 'pause') context.nfsUpdateControls('race', true);
    if (reason === 'dialog') { context.nfsUpdateControls('race', false, false); assert.equal(context.document.documentElement.dataset.controlsMode, 'menu'); }
    if (reason === 'hidden') { context.document.hidden = true; listeners.get('document:visibilitychange')(); }
    if (reason === 'blur' || reason === 'resize') listeners.get(reason)();
    left.onpointermove(event(1));
    assert.deepEqual(keys, [['ArrowLeft','keydown'], ['ArrowLeft','keyup']], reason);
  }
});

test('the ☰ button toggles the toolbar and reports its state', () => {
  const { context, toggle } = setup();
  toggle.onclick();
  assert.equal(context.document.documentElement.dataset.toolsOpen, 'true');
  assert.equal(toggle.attributes['aria-expanded'], 'true');
  toggle.onclick();
  assert.equal(context.document.documentElement.dataset.toolsOpen, 'false');
  assert.equal(toggle.attributes['aria-expanded'], 'false');
});

test('a press outside the open panel closes it, a press inside keeps it', () => {
  const { context, toggle, tools, listeners } = setup();
  toggle.onclick();
  listeners.get('document:pointerdown')({ target: tools });
  assert.equal(context.document.documentElement.dataset.toolsOpen, 'true', 'inside the panel');
  listeners.get('document:pointerdown')({ target: toggle });
  assert.equal(context.document.documentElement.dataset.toolsOpen, 'true', 'on the button');
  listeners.get('document:pointerdown')({ target: {} });
  assert.equal(context.document.documentElement.dataset.toolsOpen, 'false', 'elsewhere');
});

test('a game mode change closes the panel', () => {
  const { context, toggle } = setup();
  toggle.onclick();
  context.nfsUpdateControls('race', false);
  assert.equal(context.document.documentElement.dataset.toolsOpen, 'false');
  assert.equal(toggle.attributes['aria-expanded'], 'false');
});

test('native text entry follows the field and preserves composition until committed', () => {
  const { context, text, event } = setup();
  context.nfsTextField('name', .2, .3, .4, .05, 'OLD', 8, false);
  assert.equal(text.style.left, '200px'); assert.equal(text.style.height, '25px');
  assert.equal(text.value, 'OLD'); assert.equal(text.maxLength, 8);
  text.value = 'NEW'; text.input({ isComposing: true });
  assert.equal(context.nfsPendingText, undefined);
  text.compositionend();
  assert.equal(context.nfsPendingText.text, 'NEW');
  assert.equal(context.nfsPendingText.commit, false);
  context.nfsTextField('name', .2, .3, .4, .05, 'OLD', 8, false);
  assert.equal(text.value, 'NEW', 'frame reports must not reset an active edit');
  text.keydown({ ...event(1), key: 'Enter' });
  assert.equal(context.nfsPendingText.commit, true);
  context.nfsTextField('other', 0, 0, .2, .1, '42', 2, true);
  assert.equal(text.value, '42'); assert.equal(text.inputMode, 'numeric');
  context.nfsTextField('', 0, 0, 0, 0, '', 0, false);
  assert.equal(text.hidden, true);
});
