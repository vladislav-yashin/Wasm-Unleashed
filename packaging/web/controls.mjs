const held = new Map();
const activePointers = new Set();
function key(code, down) {
  const canvas = document.querySelector('canvas');
  if (!canvas) return;
  if (down) canvas.focus({ preventScroll: true });
  canvas.dispatchEvent(new KeyboardEvent(down ? 'keydown' : 'keyup', {code, key:code.startsWith('Key') ? code.slice(3).toLowerCase() : code === 'Space' ? ' ' : code, bubbles:true}));
}
function hold(pointer, button) {
  const previous = held.get(pointer);
  if (previous === button) return;
  held.delete(pointer);
  if (previous && ![...held.values()].includes(previous)) {
    previous.classList.remove('held');
    if (![...held.values()].some(b => b.dataset.code === previous.dataset.code)) key(previous.dataset.code, false);
  }
  if (button) {
    const already = [...held.values()].some(b => b.dataset.code === button.dataset.code);
    held.set(pointer, button); button.classList.add('held');
    if (!already) key(button.dataset.code, true);
  }
}
function releaseAll() { for (const pointer of [...held.keys()]) hold(pointer, null); activePointers.clear(); }
for (const button of document.querySelectorAll('[data-code]')) {
  button.onpointerdown = event => {
    event.preventDefault(); button.setPointerCapture(event.pointerId);
    activePointers.add(event.pointerId);
    hold(event.pointerId, button);
  };
  // Captured pointers can slide between the two arrows or between pedals.
  button.onpointermove = event => {
    if (!activePointers.has(event.pointerId)) return;
    const group = button.closest('[data-slide-group]');
    if (!group) return;
    const next = document.elementFromPoint(event.clientX, event.clientY)?.closest('[data-code]');
    hold(event.pointerId, next && group.contains(next) ? next : null);
  };
  const release = event => { activePointers.delete(event.pointerId); hold(event.pointerId, null); };
  button.onpointerup = release; button.onpointercancel = release; button.onlostpointercapture = release;
}
window.addEventListener('blur', releaseAll);
window.addEventListener('resize', releaseAll);
document.addEventListener('visibilitychange', () => { if (document.hidden) releaseAll(); });
const toolsToggle = document.querySelector('#tools-toggle');
const tools = document.querySelector('#tools');
const setToolsOpen = open => {
  document.documentElement.dataset.toolsOpen = String(open);
  toolsToggle.setAttribute('aria-expanded', String(open));
};
toolsToggle.onclick = () => {
  setToolsOpen(document.documentElement.dataset.toolsOpen !== 'true');
  releaseAll();
};
// The open panel covers the game, so a press anywhere else closes it. The press itself still
// reaches the game and the on-screen controls, so nothing is released here.
document.addEventListener('pointerdown', event => {
  if (document.documentElement.dataset.toolsOpen !== 'true') return;
  if (toolsToggle.contains(event.target) || tools?.contains(event.target)) return;
  setToolsOpen(false);
});
let lastMode;
globalThis.nfsUpdateControls = (mode, paused, driving = mode === 'race' && !paused) => {
  document.documentElement.dataset.controlsMode = driving ? 'race' : 'menu';
  const next = `${mode}:${paused}:${driving}`;
  if (next !== lastMode) {
    releaseAll(); lastMode = next;
    setToolsOpen(false);
  }
};
// Keep an actual native input over the game's focused field. Focusing it directly
// from a finger tap lets iOS/Android open the keyboard within user activation.
const textInput = document.querySelector('#text-input');
let textField;
globalThis.nfsTextField = (id, x, y, width, height, value, max, digits) => {
  if (!id) {
    textInput.hidden = true; textField = null;
    if (document.activeElement === textInput) textInput.blur();
    return;
  }
  if (textField !== id) { textInput.value = value; textField = id; }
  textInput.hidden = false;
  textInput.style.left = `${x * innerWidth}px`;
  textInput.style.top = `${y * innerHeight}px`;
  textInput.style.width = `${width * innerWidth}px`;
  textInput.style.height = `${height * innerHeight}px`;
  textInput.maxLength = max || 524288;
  textInput.inputMode = digits ? 'numeric' : 'text';
};
function sendText(commit = false) {
  globalThis.nfsPendingText = { id: textField, text: textInput.value, commit };
}
textInput.addEventListener('input', event => { if (!event.isComposing) sendText(); });
textInput.addEventListener('compositionend', () => sendText());
textInput.addEventListener('keydown', event => {
  event.stopPropagation();
  if (event.key === 'Enter' && !event.isComposing) {
    event.preventDefault(); sendText(true); textInput.blur();
    document.querySelector('canvas')?.focus({ preventScroll: true });
  }
});
textInput.addEventListener('keyup', event => event.stopPropagation());
