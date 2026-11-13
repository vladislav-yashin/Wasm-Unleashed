// Leaving the game through its own Exit command. A page cannot close its own tab, so the shell
// saves first, leaves browser fullscreen and replaces the game with the site's home page.
// `replace` keeps the finished game out of the session history, so Back cannot reopen it.
let leaving = false;

/**
 * Flush the saves, leave fullscreen and navigate to the site's home page. Re-entering the game
 * means a fresh page load; the WebAssembly instance and the event loop cannot be restarted.
 * `flush` catches its own storage failures in `bootstrap.mjs`; a rejection here is only logged.
 */
export async function leaveGame(environment = globalThis, flush = async () => {}) {
  if (leaving) return false;
  leaving = true;
  const {document, location} = environment;
  if (document?.documentElement) document.documentElement.dataset.gameMode = 'exited';
  // Release held touch / pointer buttons: the game loop is stopping, so nothing else will.
  environment.nfsUpdateControls?.('exited', false, false);
  try {
    await flush();
  } catch (error) {
    console.error('Saving before exit failed:', error);
  }
  try {
    if (document?.fullscreenElement) await document.exitFullscreen();
  } catch {
    // Fullscreen is optional (iPhone Safari has no element fullscreen).
  }
  location.replace('./');
  return true;
}
