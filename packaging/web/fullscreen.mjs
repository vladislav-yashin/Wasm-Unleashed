// Fullscreen for the browser shell. The Fullscreen API only works inside a user
// gesture, so touch devices enter fullscreen from the Start tap instead of page
// load and ask for landscape. Failures stay silent: iPhone Safari exposes no
// element fullscreen and some browsers refuse the orientation lock.
let launchAttempted = false;

/** True for touch-first devices; the same criterion the on-screen controls use. */
export const touchLayout = media => Boolean(media?.('(any-pointer:coarse)')?.matches);

/** Enter fullscreen, and landscape on touch devices; also used by the toolbar button. */
export async function enterFullscreen(environment = globalThis) {
  const {document, screen} = environment;
  if (!document?.fullscreenEnabled || document.fullscreenElement) return false;
  const coarse = touchLayout(environment.matchMedia?.bind(environment));
  try { await document.documentElement.requestFullscreen(); } catch { return false; }
  if (coarse) { try { await screen?.orientation?.lock?.('landscape'); } catch { /* unsupported */ } }
  return true;
}

/** One launch attempt per page load, on touch devices only. */
export async function requestLaunchFullscreen(environment = globalThis) {
  if (launchAttempted) return false;
  launchAttempted = true;
  return touchLayout(environment.matchMedia?.bind(environment)) && enterFullscreen(environment);
}
