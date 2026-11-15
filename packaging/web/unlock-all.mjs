/**
 * The name of the built-in unlocked profile (`UnlockAll` by default).
 *
 * It is a launch setting: the loader field publishes the normalized value to
 * `globalThis.nfsUnlockName`, and the game copies it into the save layer before it reads the save
 * directory (`crates/open-unleashed/src/web.rs`). An existing profile is never renamed; the new
 * name creates/uses the profile with that stem.
 */

export const DEFAULT_NAME = 'UnlockAll';
export const STORAGE_KEY = 'wasm-unleashed-unlock-name';
export const MAX_LENGTH = 16;

// The shared profile-box filter (`layout/input.rs` `type_char`): `%` is a `printf` format, the
// rest is the set of characters Windows refuses in a file name.
const ILLEGAL = new Set(['%', '\\', '/', ':', '*', '?', '"', '<', '>', '|']);

function defaultStorage() {
  try { return globalThis.localStorage ?? null; } catch { return null; }
}

/**
 * The in-game profile box's rules: 1…16 characters in `0x20..0xFE`, no `%` and none of the nine
 * file-name characters. The `.sav` stores one byte per character, so anything above Latin-1 falls
 * back to the default rather than being silently truncated.
 */
export function normalizeUnlockName(value) {
  const name = String(value ?? '').trim();
  if (!name || [...name].length > MAX_LENGTH) return DEFAULT_NAME;
  for (const char of name) {
    const code = char.codePointAt(0);
    if (code < 0x20 || code >= 0xFF || ILLEGAL.has(char)) return DEFAULT_NAME;
  }
  return name;
}

/** The saved name, or the default when storage is unavailable or holds an invalid value. */
export function storedUnlockName(storage = defaultStorage()) {
  try { return normalizeUnlockName(storage?.getItem(STORAGE_KEY)); } catch { return DEFAULT_NAME; }
}

/** Publish the normalized name to the game and remember it; returns the accepted name. */
export function applyUnlockName(value, { storage = defaultStorage(), target = globalThis } = {}) {
  const name = normalizeUnlockName(value);
  target.nfsUnlockName = name;
  try { storage?.setItem(STORAGE_KEY, name); } catch { /* storage may be unavailable */ }
  return name;
}
