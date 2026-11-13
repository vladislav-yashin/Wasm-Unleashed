import './controls.mjs';
import { enterFullscreen, requestLaunchFullscreen } from './fullscreen.mjs';
import { AssetSource } from './assets.mjs';
import { VirtualFileSystem } from './vfs.mjs';
import { apply, detectLocale, getLocale, onLocaleChange, setLocale, t } from './i18n.mjs';
import { applyScale, storedScale } from './render-scale.mjs';
const status = document.querySelector('#status');
const loader = document.querySelector('#loader');
const start = document.querySelector('#start');
const progress = document.querySelector('#progress');
const saveStatus = document.querySelector('#save-status');
setLocale(detectLocale(), { persist: false });
apply();
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
// Texts that depend on runtime state: re-render them when the locale changes.
const dynamic = new Map();
const say = (node, render) => { dynamic.set(node, render); node.textContent = render(); };
const message = (key, params) => () => t(key, params);
onLocaleChange(() => { apply(); for (const [node, render] of dynamic) node.textContent = render(); });
let fs, initialized = false;
let recovery = false;
const report = error => {
  recovery ||= initialized || !!document.querySelector('canvas');
  say(status, () => String(error.message || error)); loader.hidden = false;
  say(start, () => t(recovery ? 'action.reload' : 'action.retry'));
  if (recovery) { flush(); start.focus(); }
};
globalThis.nfsFatal = text => report(new Error(text));
window.addEventListener('unhandledrejection', event => report(event.reason));
window.addEventListener('error', event => report(event.error || event.message));
let timer;
let retryTrack;
async function flush() {
  if (!fs?.dirty.size) return;
  say(saveStatus, message('saves.saving'));
  try { await fs.flush(); say(saveStatus, message(fs.dirty.size ? 'saves.saving' : 'saves.saved')); }
  catch (error) { say(saveStatus, message('saves.failed', {error: error.message})); }
}
start.addEventListener('click', async () => {
  if (recovery) { location.reload(); return; }
  if (retryTrack) { const resume = retryTrack; retryTrack = null; resume(); return; }
  // Fullscreen needs this user gesture, so ask before the first await.
  void requestLaunchFullscreen();
  start.disabled = true;
  try {
    if (!fs) {
      const source = await AssetSource.open(new URLSearchParams(location.search).get('data') || './data/');
      fs = new VirtualFileSystem(source);
      globalThis.nfsVfs = fs;
      try { await fs.openSaves(); } catch (error) { say(saveStatus, message('saves.temporary', {error: error.message})); }
    }
    say(status, message('loading.assets'));
    const tracks = Object.keys(fs.source.manifest.files).filter(path => path.startsWith('gamedata/track/'));
    const sharedTrack = path => path.endsWith('/animdefs.txt') || path.endsWith('/default.bin') || path.startsWith('gamedata/track/data/');
    const trackPaths = name => tracks.filter(path => sharedTrack(path) || !name || path.split('/').at(-1).startsWith(name.toLowerCase()));
    globalThis.nfsTrackReady = name => trackPaths(name).every(path => fs.files.has(`game/${path}`));
    globalThis.nfsPrepareTrack = async name => {
      loader.hidden = false;
      while (true) {
        start.hidden = true;
        try {
          await fs.preload(trackPaths(name), (done,total,path) => { progress.max=total;progress.value=done;say(status, message('loading.progress', {done, total, path})); });
          loader.hidden = true; start.hidden = false;
          return;
        } catch(error) {
          say(status, () => String(error.message || error)); loader.hidden = false; say(start, message('action.retry')); start.hidden = false; start.disabled = false;
          await new Promise(resolve => { retryTrack = resolve; });
        }
      }
    };
    const initial = Object.keys(fs.source.manifest.files).filter(path => !path.startsWith('gamedata/track/') || sharedTrack(path) || path.split('/').at(-1).startsWith('alps'));
    await fs.preload(initial, (done, total, path) => {
      progress.max = total; progress.value = done;
      say(status, message('loading.progress', {done, total, path}));
    });
    if (!initialized) {
      const { default: init } = await import('./pkg/open_unleashed.js');
      await init();
      initialized = true;
      timer = setInterval(flush, 1000);
    }
    loader.hidden = true;
    document.querySelector('canvas')?.focus();
  } catch (error) { report(error); }
  finally { start.disabled = false; }
});
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
