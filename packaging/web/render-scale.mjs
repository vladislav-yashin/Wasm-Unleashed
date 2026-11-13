// The browser shell's render scale: the fraction of the viewport's pixels the game draws at.
// `style.css` stretches the canvas over the whole viewport, so a lower scale is upscaled by the
// browser and trades sharpness for frame rate. The game reads `globalThis.nfsRenderScale` when it
// sizes its surface (`crates/open-unleashed/src/web.rs`). This is a platform setting: it never
// changes the game's own Graphics options.
export const STORAGE_KEY = 'wasm-unleashed-render-scale';
export const SCALES = [1, 0.75, 0.5, 0.4];

function defaultStorage() {
  try { return globalThis.localStorage ?? null; } catch { return null; }
}

/** A shipped scale, else full resolution (an unknown or missing value is not an error). */
export function normalizeScale(value) {
  const scale = Number(value);
  return SCALES.includes(scale) ? scale : 1;
}

/** The saved scale, or 1 when storage is unavailable or holds an unknown value. */
export function storedScale(storage = defaultStorage()) {
  try { return normalizeScale(storage?.getItem(STORAGE_KEY)); } catch { return 1; }
}

/** Publish the scale to the game and remember it; returns the normalized value. */
export function applyScale(value, { storage = defaultStorage(), target = globalThis } = {}) {
  const scale = normalizeScale(value);
  target.nfsRenderScale = scale;
  try { storage?.setItem(STORAGE_KEY, String(scale)); } catch { /* storage may be unavailable */ }
  return scale;
}
