import './controls.mjs';
import { enterFullscreen, requestLaunchFullscreen } from './fullscreen.mjs';
import { AssetSource } from './assets.mjs';
import { VirtualFileSystem } from './vfs.mjs';
import { carPaths, gamePaths, initialPaths } from './startup-assets.mjs';
import { apply, detectLocale, getLocale, onLocaleChange, setLocale, t } from './i18n.mjs';
import { applyScale, storedScale } from './render-scale.mjs';
import { applyUnlockName, normalizeUnlockName, storedUnlockName } from './unlock-all.mjs';
import { leaveGame } from './exit-game.mjs';
import { installNetwork } from './net.mjs';
const status = document.querySelector('#status');
const loader = document.querySelector('#loader');
const start = document.querySelector('#start');
const progress = document.querySelector('#progress');
const saveStatus = document.querySelector('#save-status');
installNetwork();
setLocale(detectLocale(), { persist: false });
apply();
// The game draws the original race loading screen (0x4A4A70) while the shell fetches a launch's
// track files. `nfsTrackScreen` hides this panel then — it comes back by itself for an error and
// its Retry — and `nfsTrackProgress` is the share of the files prepared, which moves that
// screen's bar.
let trackProgress = 0;
globalThis.nfsTrackProgress = () => trackProgress;
globalThis.nfsTrackScreen = active => {
  if (!active) return;
  loader.hidden = true;
  loader.setAttribute('aria-busy', 'false');
};
const language = document.querySelector('#language');
language.value = getLocale();
language.onchange = () => { setLocale(language.value); };
// The render scale is the shell's own setting: the game reads it when it sizes its surface, so it
// applies from the next frame on, before or after Start.
const renderScale = document.querySelector('#render-scale');
if (renderScale) {
  renderScale.value = String(storedScale());
  applyScale(renderScale.value);
  renderScale.onchange = () => { applyScale(renderScale.value); };
}
// The built-in unlock-all profile's name is a launch setting: the game copies
// `globalThis.nfsUnlockName` into the save layer before it creates the profile. The field is in
// the loader because the toolbar sits under the loader until the game has started; changing the
// value later applies at the next start, and an existing profile is never renamed.
const unlockName = document.querySelector('#unlock-name');
if (unlockName) {
  unlockName.value = applyUnlockName(storedUnlockName());
  unlockName.oninput = () => { globalThis.nfsUnlockName = normalizeUnlockName(unlockName.value); };
  unlockName.onchange = () => { unlockName.value = applyUnlockName(unlockName.value); };
  unlockName.addEventListener('keydown', event => {
    event.stopPropagation();
    if (event.key !== 'Enter' && event.key !== 'Escape') return;
    event.preventDefault();
    unlockName.value = applyUnlockName(unlockName.value);
    unlockName.blur();
    document.querySelector('canvas')?.focus({ preventScroll: true });
  });
  unlockName.addEventListener('keyup', event => event.stopPropagation());
}
// Texts that depend on runtime state: re-render them when the locale changes.
const dynamic = new Map();
const say = (node, render) => { dynamic.set(node, render); node.textContent = render(); };
const message = (key, params) => () => t(key, params);
onLocaleChange(() => { apply(); for (const [node, render] of dynamic) node.textContent = render(); });
let fs, initialized = false;
let recovery = false;
let launching = false;
let loadingFailure;
let rejectReady;
const count = document.querySelector('#loading-count');
const percent = document.querySelector('#loading-percent');
const details = document.querySelector('#loading-details');
const errorText = document.querySelector('#loading-error');
const steps = [...document.querySelectorAll('[data-step]')];
function showLoading(stage, key, { compact = false } = {}) {
  loader.dataset.presentation = compact ? 'compact' : 'startup';
  loader.setAttribute('aria-labelledby', compact ? 'status' : 'loading-title');
  loader.hidden = false;
  loader.dataset.state = 'loading';
  loader.setAttribute('aria-busy', 'true');
  details.hidden = true;
  details.open = false;
  errorText.textContent = '';
  start.hidden = true;
  if (unlockName) unlockName.disabled = true;
  progress.removeAttribute('value');
  count.textContent = ''; dynamic.delete(count);
  percent.textContent = '…';
  const index = steps.findIndex(step => step.dataset.step === stage);
  steps.forEach((step, i) => {
    if (i === index) step.setAttribute('aria-current', 'step');
    else step.removeAttribute('aria-current');
    step.dataset.complete = String(i < index);
  });
  say(status, message(key));
}
function showProgress(done, total) {
  progress.max = Math.max(total, 1);
  progress.value = done;
  percent.textContent = `${total ? Math.floor(done / total * 100) : 0}%`;
  say(count, message('loading.prepared', {done, total}));
}
function finishLoading() {
  loader.hidden = true;
  loader.setAttribute('aria-busy', 'false');
  document.querySelector('canvas')?.focus();
}
// wasm-bindgen initialization starts the asynchronous renderer. Keep the loading
// screen until the application reports a playable state, rather than flashing black.
function waitForGame() {
  if (loadingFailure) return Promise.reject(loadingFailure);
  const ready = () => ['intro', 'menu', 'race'].includes(document.documentElement.dataset.gameMode);
  if (ready()) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const observer = new MutationObserver(() => {
      if (!ready()) return;
      observer.disconnect(); rejectReady = null; resolve();
    });
    rejectReady = error => { observer.disconnect(); rejectReady = null; reject(error); };
    observer.observe(document.documentElement, {attributes:true, attributeFilter:['data-game-mode']});
  });
}
const report = (error, {compact = false} = {}) => {
  const failure = error instanceof Error ? error : new Error(String(error));
  loadingFailure = failure;
  rejectReady?.(failure);
  if (!compact) {
    recovery ||= initialized || !!document.querySelector('canvas');
    loader.dataset.presentation = 'startup';
    loader.setAttribute('aria-labelledby', 'loading-title');
  }
  loader.dataset.state = 'error';
  if (!progress.hasAttribute('value')) progress.value = 0;
  percent.textContent = '';
  loader.setAttribute('aria-busy', 'false');
  say(status, message('loading.failed'));
  errorText.textContent = failure.message;
  details.hidden = false;
  start.hidden = false;
  start.disabled = false;
  if (unlockName) unlockName.disabled = false;
  loader.hidden = false;
  say(start, () => t(recovery ? 'action.reload' : 'action.retry'));
  if (recovery) flush();
  start.focus();
};
globalThis.nfsFatal = text => report(new Error(text));
window.addEventListener('unhandledrejection', event => report(event.reason));
window.addEventListener('error', event => report(event.error || event.message));
let timer;
let retryResources;
async function flush() {
  if (!fs?.dirty.size) return;
  say(saveStatus, message('saves.saving'));
  try { await fs.flush(); say(saveStatus, message(fs.dirty.size ? 'saves.saving' : 'saves.saved')); }
  catch (error) { say(saveStatus, message('saves.failed', {error: error.message})); }
}
// The in-game Exit command (`web.rs`'s `nfsExitGame`): save, leave fullscreen and return to the
// site's home page. The WebAssembly instance cannot be restarted, so re-entering reloads the page.
globalThis.nfsExitGame = () => { void leaveGame(globalThis, flush); };
async function launch({ gesture = false } = {}) {
  if (launching && !retryResources && !recovery) return;
  if (recovery) { location.reload(); return; }
  if (retryResources) { const resume = retryResources; retryResources = null; resume(); return; }
  // Fullscreen needs this user gesture, so ask before the first await.
  if (gesture) void requestLaunchFullscreen();
  launching = true;
  loadingFailure = null;
  start.disabled = true;
  // Freeze the launch setting before the first await; the game reads it at wasm start-up.
  if (unlockName) unlockName.value = applyUnlockName(unlockName.value);
  showLoading('connect', 'loading.connecting');
  try {
    // Download and compile without starting Rust: its synchronous reads need the VFS.
    // Handle rejection immediately while resource preparation is still running.
    const engine = Promise.all([
      import('./pkg/open_unleashed.js'),
      (async () => {
        const response = await fetch(new URL('./pkg/open_unleashed_bg.wasm', import.meta.url));
        if (!response.ok) throw new Error(`WebAssembly HTTP ${response.status}`);
        if (WebAssembly.compileStreaming) {
          try { return await WebAssembly.compileStreaming(response.clone()); }
          catch { /* Hosts without the WASM MIME type need the byte-buffer path. */ }
        }
        return WebAssembly.compile(await response.arrayBuffer());
      })(),
    ]).then(value => ({value}), error => ({error}));
    if (!fs) {
      const source = await AssetSource.open(new URLSearchParams(location.search).get('data') || './data/');
      fs = new VirtualFileSystem(source);
      globalThis.nfsVfs = fs;
      try { await fs.openSaves(); } catch (error) { say(saveStatus, message('saves.temporary', {error: error.message})); }
    }
    showLoading('resources', 'loading.assets');
    const tracks = Object.keys(fs.source.manifest.files).filter(path => path.startsWith('gamedata/track/'));
    const sharedTrack = path => path.endsWith('/animdefs.txt') || path.endsWith('/default.bin') || path.startsWith('gamedata/track/data/');
    // Race setup still chooses opponents/traffic synchronously. Prepare their
    // possible models here; showroom previews prepare only the displayed family.
    const raceCars = carPaths(fs.source.manifest.files);
    const trackPaths = name => [...raceCars, ...tracks.filter(path => sharedTrack(path) || !name || path.split('/').at(-1).startsWith(name.toLowerCase()))];
    globalThis.nfsTrackReady = name => trackPaths(name).every(path => fs.files.has(`game/${path}`));
    const prepareResources = async (paths, key, progress = showProgress) => {
      while (true) {
        showLoading('resources', key, {compact:true});
        try {
          trackProgress = 0;
          await fs.preload(paths, progress);
          trackProgress = 1;
          finishLoading();
          return;
        } catch(error) {
          report(error, {compact:true});
          await new Promise(resolve => { retryResources = resolve; });
        }
      }
    };
    // The track's own screen moves its bar with the files prepared so far (0x4A4A70's steps), so
    // the shell reports the share instead of drawing its compact panel over the game.
    const showTrackProgress = (done, total) => {
      trackProgress = total ? done / total : 0;
      showProgress(done, total);
    };
    globalThis.nfsPrepareTrack = name => prepareResources(trackPaths(name), 'loading.track', showTrackProgress);
    globalThis.nfsFilesReady = paths => gamePaths(paths).every(path => fs.files.has(`game/${path}`));
    globalThis.nfsPrepareFiles = paths => prepareResources(gamePaths(paths), 'loading.assets');
    const initial = initialPaths(fs.source.manifest.files);
    await fs.preload(initial, showProgress);
    showLoading('engine', 'loading.engine');
    if (!initialized) {
      const prepared = await engine;
      if (prepared.error) throw prepared.error;
      const [{ default: init }, module] = prepared.value;
      await init({module_or_path: module});
      initialized = true;
      timer = setInterval(flush, 1000);
    }
    await waitForGame();
    finishLoading();
  } catch (error) { report(error); }
  finally { launching = false; start.disabled = false; }
}
start.addEventListener('click', () => { void launch({gesture:true}); });
document.querySelector('#fullscreen').onclick = async () => {
  try {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await enterFullscreen();
    document.querySelector('canvas')?.focus();
  } catch { /* optional on mobile Safari */ }
};
document.querySelector('#export').onclick = () => {
  if (!fs) return;
  const url = URL.createObjectURL(new Blob([fs.exportSaves()], {type:'application/json'}));
  const link = document.createElement('a'); link.href = url; link.download = 'wasm-unleashed-saves.json'; link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
};
document.querySelector('#import').onchange = async event => {
  const file = event.target.files[0];
  if (!file || !fs) return;
  try { await fs.importSaves(await file.text()); say(saveStatus, message('saves.imported')); }
  catch (error) { say(saveStatus, () => error.message); }
  event.target.value = '';
};
document.addEventListener('visibilitychange', () => { if (document.hidden) flush(); });
window.addEventListener('pagehide', flush);
const observedCanvases = new WeakSet();
// The canvas's backing-store size, which the render scale changes (wgpu sets `width`/`height`).
// It is the real number of pixels the game draws, so the label is the honest readout.
const renderSize = document.querySelector('#render-size');
const showRenderSize = canvas => { if (renderSize && canvas.width > 1 && canvas.height > 1) renderSize.textContent = `${canvas.width}×${canvas.height}`; };
new MutationObserver(() => {
  const canvas = document.querySelector('canvas');
  if (!canvas || observedCanvases.has(canvas)) return;
  observedCanvases.add(canvas);
  canvas.addEventListener('webglcontextlost', event => {
    event.preventDefault(); flush(); report(new Error(t('error.contextLost')));
  });
  new MutationObserver(() => showRenderSize(canvas)).observe(canvas, {attributes:true, attributeFilter:['width', 'height']});
  showRenderSize(canvas);
}).observe(document.body, {childList:true});
