# Wasm Unleashed browser build

The engine compiles to `wasm32-unknown-unknown` and is delivered as a browser game. Original resources are
served via HTTP and presented to existing parsers through a preloaded virtual filesystem.
Basic Firefox checks with original resources passed: main menu, profile creation
and reload, quick race and Factory Driver launch, keyboard/touch controls, pause,
Web Audio and original intro video. Full playthroughs and real mobile/WebGPU
hardware acceptance remain manual.

## Build and serve

Use Rust stable >= 1.95, Python >= 3.11 and Node >= 22 for the JS tests.

```sh
rustup target add wasm32-unknown-unknown --toolchain stable
cargo +stable install wasm-bindgen-cli --version 0.2.129 --locked
python3 tools/web/build.py
# The target stages GameData, FEData, root configuration and derives the movie
# tables. It validates the installation's executables and uses the first
# compatible one.
make web-data GAME_DIR=/path/to/original/game
# Optional: override the movie executable and add a root file outside GAME_DIR:
make web-data GAME_DIR=/path/to/game MOVIE_EXE=/path/to/original/Porsche.exe EXTRA_FILES=/path/to/original/extra.txt
# Start from the repository root:
docker compose -f packaging/web/compose.yaml up --build
```

`make help` lists the local build, serve, docker, test and deployment-validation
targets, including `make build-web`, `make serve`, `make test` and
`make check-deploy`.

Open http://localhost:8080. To test from a phone on the same LAN, run
`python3 tools/web/serve.py --host 0.0.0.0` and open `http://<computer-LAN-IP>:8080`.
Plain HTTP uses WebGL2 and a JavaScript SHA-256 fallback for asset verification;
the persistent asset cache and secure-context APIs such as WebGPU and gamepads
require HTTPS (or localhost). Use HTTPS for deployed sites.
For local development without Docker, run `python3 tools/web/serve.py` and open
http://127.0.0.1:8080. For production, use the blue-green deployment described in
[`DEPLOY.md`](DEPLOY.md): the image is served on loopback and Caddy terminates
HTTPS. Mount the prepared `web-data/` directory at `/data/`. Never serve the
repository or original installation directly. `?data=https://example.test/data/`
changes the resource base URL; a different origin must supply CORS headers.

`build.py --debug` produces a larger development build. The build checks that the
wasm-bindgen CLI version matches Cargo.lock. `smoke.html` initializes the renderer
without original game resources; it is a diagnostic page, not a gameplay test.

## Loading and saves

The manifest contains case-insensitive paths, sizes, groups and SHA-256 hashes.
Content-addressed objects are independently fetched, verified and cached.
The `--from-installation` path copies only `GameData/`, `FEData/`, `fe.txt`,
`version.txt`, `nfs5.ini` (when present), and the derived `madtables.bin`; other
root files such as the original executable, DLLs or `Backup/` are ignored.
`--movie-exe` extracts the small `madtables.bin` using the existing Rust parser;
the executable itself is not published. Without these tables original videos
are skipped. Original data is not included in this repository or the web build.

The initial load includes front-end, cars, simulation, short sounds and the starting
Alps track. Quick races load the selected track before changing game state;
series/career modes currently preload the remaining tracks before their first
race. Shared animation and sky metadata are included in the bootstrap. Music archives are segmented: only the directory and per-song headers are loaded
initially, and individual songs are fetched and verified when playback starts.
Car models and short sound banks are currently part of the initial load.
Loaded resources remain in memory until page reload; explicit unload is exposed
by the VFS but not yet integrated with game-state ownership.

Profiles, records and settings write into `/saves`, with periodic IndexedDB
transactions. The toolbar reports whether writes reached persistent storage and
supports JSON export/import. Import requires restarting the game to reload its
in-memory profile. If persistence is unavailable, export before closing the page.

Keyboard, Browser Gamepad API and touch buttons feed existing controls. On-screen
controls switch between a menu D-pad with OK/Back and larger race steering/pedal
buttons. Slide between steering buttons or pedals without lifting the finger.
On touch devices, the lower HUD instruments move above the controls and the ☰
button opens the save/language/fullscreen toolbar. With a mouse the toolbar stays
open in the top right corner in menus and folds into a ☰ at the bottom centre
during a race, where the HUD leaves the strip free; its panel opens upwards above
the button. The first **Start** tap also
enters browser fullscreen and requests landscape orientation; the ⛶ button still
toggles the mode manually, and browsers without element fullscreen — currently
Safari on iPhone — keep the windowed page. Tap the active in-game text field to
open the OS keyboard; the game still applies its original character and length
restrictions.

The toolbar's render-scale selector (100 / 75 / 50 / 40%) sets the fraction of the viewport the
canvas is drawn at, and the label beside it shows the canvas's real backing-store size. CSS
stretches the canvas over the whole page, so a lower scale is a softer but cheaper frame; the
choice is remembered in `localStorage` (`wasm-unleashed-render-scale`) and applies from the next
frame. It is a platform setting and independent of the game's own `Graphics` screen — in a browser
that screen's `Resolutions` list has one row (the canvas) and cannot change the page's size.
Browser audio uses CPAL's Web Audio output, with asynchronous music feeding and
resume on user interaction. It does not yet use a dedicated AudioWorklet mixer.
Wheel force feedback and network sessions are unavailable in the web version.

## Interface language

The browser shell — loader, toolbar, touch labels and status messages — is translated in
`packaging/web/i18n.mjs`. English is the default and the fallback; `ru`, `de`, `fr`, `it` and `es`
ship alongside it. The language comes from `?lang=de`, then a saved choice in `localStorage`
(`wasm-unleashed-locale`), then the browser's `navigator.languages`, then English. A query
parameter applies to that page load only; the toolbar selector stores an explicit choice.
`index.html` keeps the English text so the page is usable before the module loads. To add a locale,
add its catalog, its `<option>` and the tests that compare key sets. Errors raised by `assets.mjs`,
`vfs.mjs` and IndexedDB stay English inside the translated wrapper.

This setting is independent of the game's own text: the front end, HUD and subtitles still come
from the original tables (`FEStrings.loc`, the install's `Language` registry value), so switching
the shell does not retranslate them.

## Validation and remaining acceptance work

```sh
cargo +stable check -p open-unleashed --target wasm32-unknown-unknown --locked
cargo +stable test -p nfs-formats -p nfs-net --lib
python3 -m unittest discover -s tools/web -p 'test_*.py'
node --test packaging/web/assets.test.mjs packaging/web/vfs.test.mjs packaging/web/i18n.test.mjs packaging/web/controls.test.mjs packaging/web/fullscreen.test.mjs packaging/web/render-scale.test.mjs
```

For Firefox frame-pacing investigations, pass `--profile-webgl` to
`tools/web/browser_check.py` with a running Marionette-enabled Firefox. The check
writes `webgl-profile.json` under its output directory, recording frame gaps,
WebGL call durations and long animation/timer callbacks. With `--skip-intro`,
measurement starts after the intro skip. Instrumentation adds overhead and is
only enabled by this diagnostic option; it is not part of the shipped game.

Remaining acceptance: a complete quick race, career and mission, audio continuity,
mobile memory/performance, context loss/recovery and persistent save round trips
on real browsers with full original resources. Browser photos download the current canvas resolution.
Future multiplayer needs a WebSocket transport and a UDP gateway; the existing
wire protocol remains separate from browser asset delivery.
