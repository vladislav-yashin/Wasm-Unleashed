# First start in the browser

**English** | [Русский](../ru/install.md)

## What you need

- Your own copy of **Need for Speed: Porsche Unleashed** (PC, 2000). The game data is prepared by whoever
  hosts the build; nothing of the original game is included in the project.
- A current desktop or mobile browser with WebAssembly, WebGL2 or WebGPU, IndexedDB and Web Audio. WebGPU is
  used when the browser provides it; otherwise the renderer falls back to WebGL2.
- Use HTTPS for hosted sites. For a phone on the same LAN, run
  `python3 tools/web/serve.py --host 0.0.0.0` and open `http://<computer-LAN-IP>:8080`.
  Plain HTTP uses WebGL2 and fallback SHA-256 verification; WebGPU, gamepads and the persistent
  asset cache require HTTPS or localhost. If game assets come from another origin, that server
  must send CORS headers.

## Opening the game

1. Open the hosted build, or start your own server (see [building](building.md)).
2. Press **Start**. The page downloads the game resources and shows the progress.
3. Wait for the loader to finish. The intro movies and the original front end start as in the original game.
4. On touch devices the first **Start** tap enters browser fullscreen and asks for landscape
   orientation. The fullscreen button in the top-right corner toggles the mode. The game canvas
   takes the whole page.
5. The language selector in the toolbar switches the browser shell and remembers your choice; the shell
   follows the browser's language until then. The game's own text still comes from the installed data.
6. To leave the game, use **Exit** in its front end and confirm. A browser page cannot close its own
   tab, so the shell saves the progress, leaves browser fullscreen and returns to the site's home
   page. Quitting a race from its pause menu returns to the game's front end, as in the original.

The game data is served over HTTP and is read-only. Only progress and settings are written by the browser.

## Saves, settings and the toolbar

Profiles, records and settings live in the browser's IndexedDB under `/saves`. The toolbar reports whether
writes reached persistent storage. Use **Export saves** to download a JSON copy and **Import** to load one
back. Import replaces the stored saves; restart the game afterwards so the in-memory profile is reloaded.

If persistent storage is unavailable (for example, private browsing or a browser that refused the storage
request), the game keeps progress for the current page only. Export the saves before closing the page.

## Network games

Browser multiplayer is planned for a later stage. Network races, LAN discovery and the original UDP protocol
are not available in the browser build yet, so no firewall ports need to be opened.

## If something goes wrong

- **The loader reports a missing or invalid resource** — the server does not have the prepared `web-data`
  directory, the manifest is incomplete, or an object failed its hash check. Rebuild the data with
  `make web-data GAME_DIR=/path/to/game` and upload the whole output.
- **Resources fail to load from another origin** — the asset server needs HTTPS and CORS headers.
- **The game does not start or shows a graphics error** — the browser could not create a WebGPU or WebGL2
  device. Update the browser and the graphics driver, enable hardware acceleration, or try another browser.
- **The intro is skipped** — the server's `web-data` has no `madtables.bin`. It is derived from a compatible
  original executable; see [game data](game-data.md).
- **The graphics context is lost** — the page asks to reload. Saved progress is flushed before the reload and
  is restored from IndexedDB.
- **A problem needs reporting** — open the browser's developer console and include its errors together with
  the browser, device and GPU model in the issue ([contributing](../../CONTRIBUTING.md)).
