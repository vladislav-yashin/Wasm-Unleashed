//! Leaving the program: the one exit path shared by the front end, the pause menu and replay playback.

use crate::*;

impl App {
    /// The original's own ways out of the program: the front end's `QUIT` dialog, a back that
    /// closes its last layer, the pause menu's quit when there is no front end to return to, and a
    /// replay that ends without one.
    ///
    /// A browser cannot close its own tab, so the shell is told first: it flushes the saves, leaves
    /// fullscreen and replaces the game with the site's home page. Scripted and diagnostic runs
    /// keep calling `event_loop.exit` directly — they are not a player's exit.
    pub(crate) fn quit_game(&mut self, event_loop: &ActiveEventLoop) {
        self.input.release_all();
        // stop Web Audio before the event loop goes away; the shell may wait a moment for IndexedDB
        self.snd.audio = None;
        self.snd.music = None;
        #[cfg(target_arch = "wasm32")]
        crate::web::exit_game();
        event_loop.exit();
    }
}
