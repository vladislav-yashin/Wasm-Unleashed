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
The packer also writes deterministic `.gz` sidecars when compression saves space.
Nginx and the local server negotiate these using `Accept-Encoding`; browsers
transparently decompress before SHA-256 verification. Re-run asset preparation to
add sidecars to an existing data directory and keep them alongside the objects
when deploying. Uncompressed objects remain available for older hosts and clients.
WebAssembly downloads and compiles alongside asset preparation after Start;
the engine starts only once the virtual filesystem and saves are ready.
The `--from-installation` path copies only `GameData/`, `FEData/`, `fe.txt`,
`version.txt`, `nfs5.ini` (when present), and the derived `madtables.bin`; other
root files such as the original executable, DLLs or `Backup/` are ignored.
`--movie-exe` extracts the small `madtables.bin` using the existing Rust parser;
the executable itself is not published. Without these tables original videos
are skipped. Original data is not included in this repository or the web build.

The initial load includes front-end tables/art, simulation, short sounds, the
starting Alps track, the default 993 model, the two original intro movies and the
entry screen's `ch6` movie. Other movies, Chronicle photographs and Showcase packs
are fetched only when a visible widget needs them; only the selected file or
thumbnail set is prepared before its synchronous decoder runs. The loader blocks
input and pauses front-end frames until preparation succeeds, with Retry on errors.
Showroom car previews similarly prepare only the displayed model families.
These deferred loads work with existing manifests; asset repacking is not required.
Quick races load the selected track before changing game state;
series/career modes currently preload the remaining tracks before their first
race. Shared animation and sky metadata are included in the bootstrap. Music archives are segmented: only the directory and per-song headers are loaded
initially, and individual songs are fetched and verified when playback starts.
The remaining car models are prepared before the first race because opponent and
traffic selection is still synchronous; short sound banks remain in the initial load.
Loaded resources remain in memory until page reload; explicit unload is exposed
by the VFS but not yet integrated with game-state ownership.

Profiles, records and settings write into `/saves`, with periodic IndexedDB
transactions. Profile names are limited to 16 characters. The first start creates
an `UnlockAll` profile (`0UnlockAll.sav`) that is offered in the chooser but not
selected automatically; it opens every track, unique car and tournament with
maximum credits. The toolbar reports whether writes reached persistent storage and
supports JSON export/import. Import requires restarting the game to reload its
in-memory profile. If persistence is unavailable, export before closing the page.
**Exit** in the game's own front end flushes the saves, leaves browser fullscreen and returns
to the site's home page; the page cannot close its own tab. Quitting a race from its pause
menu returns to the game's front end.

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
Wheel force feedback is unavailable in the web version. Network sessions can join a native game
host at an IPv4 address through the relay described below; hosting and LAN discovery remain native.
Keep the game tab visible while a network race runs: browsers throttle a hidden tab, the game stops
polling, and the host classifies the player as gone. Switching to another window no longer pauses a
network race (a single-player race still pauses).
Controller rumble is driven through the browser's Gamepad API in Chrome, Edge and Safari 16.4+;
Firefox does not expose gamepad rumble yet. The pad does not buzz with the engine: rough shoulders,
verges and grass rumble, tyre scrub and wheelspin are felt near the grip limit, locked front wheels
pulse while braking, and a gear change gives a short kick.

## Interface language

The browser shell — loader, toolbar, touch labels and status messages — is translated in
`packaging/web/i18n.mjs`. English is the default and the fallback; `ru`, `de`, `fr`, `it` and `es`
ship alongside it. The language comes from `?lang=de`, then a saved choice in `localStorage`
(`wasm-unleashed-locale`), then English. Browser language preferences do not
change this default. A query
parameter applies to that page load only; the toolbar selector stores an explicit choice.
`game.html` keeps the English text so the page is usable before the module loads. To add a locale,
add its catalog, its `<option>` and the tests that compare key sets. Errors raised by `assets.mjs`,
`vfs.mjs` and IndexedDB stay English inside the translated wrapper.

This setting is independent of the game's own text: the front end, HUD and subtitles still come
from the original tables (`FEStrings.loc`, the install's `Language` registry value), so switching
the shell does not retranslate them.

## Validation and remaining acceptance work

```sh
cargo +stable check -p open-unleashed --target wasm32-unknown-unknown --locked
cargo +stable test -p nfs-formats -p nfs-net --lib -p nfs-relay
python3 -m unittest discover -s tools/web -p 'test_*.py'
node --test packaging/web/assets.test.mjs packaging/web/vfs.test.mjs packaging/web/i18n.test.mjs packaging/web/controls.test.mjs packaging/web/fullscreen.test.mjs packaging/web/render-scale.test.mjs packaging/web/exit-game.test.mjs packaging/web/startup-assets.test.mjs packaging/web/net.test.mjs
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

## Network play

A browser cannot open a UDP socket, so the browser build speaks the original protocol through a
WebSocket. `nfs-relay` (`tools/net-relay`) accepts `/relay?target=ip:port`, validates the target,
opens one connected UDP socket to the game host's port 9442 and forwards the datagrams unchanged.
One WebSocket is one peer: the host sees each browser player as an ordinary UDP client.

The peer-to-peer screen (`PeerToPeerIP` → `Comms.IP` → `CONNECT`) is the supported path. `CREATE`,
the LAN games list and the modem screen stay native-only; the browser disables those buttons. The
typed destination must be an IPv4 literal (there is no browser DNS lookup on this path).

Locally, `make serve` builds and starts the relay and proxies `/relay` to it, so the page's
same-origin WebSocket URL works without a query parameter:

```sh
make serve
# open http://127.0.0.1:8080/game.html
```

The relay's default policy refuses loopback and private addresses; the development server passes
`--allow-loopback --allow-private` for this local case only. To reach a native host on another
machine, use its LAN address. Inside Docker, `127.0.0.1` means the relay container itself, so use
the host's LAN address (or run the relay natively for a same-machine test).

`python3 tools/web/serve.py` without `--relay` is still a plain static server. To use a standalone
relay instead, run `make relay` and point the page at it explicitly:

```sh
make relay
python3 tools/web/serve.py
# open http://127.0.0.1:8080/game.html?relay=ws://127.0.0.1:8090/relay
```

`--relay-port` makes the built-in proxy point at an existing relay without starting one.

The Docker stack (`make docker-up`) builds the relay and nginx proxies same-origin `/relay` to it,
so no query parameter is needed. The browser always uses `wss:` on an HTTPS page.

Operators must set `RELAY_ALLOWED_ORIGINS` to the site's origin. The relay only forwards to the
configured UDP port (9442 by default) and to global unicast addresses: loopback, link-local, private
networks, carrier-grade NAT and the other special-purpose ranges are refused unless the operator
names a `RELAY_ALLOW_CIDRS` allowlist (which replaces that rule) or sets `RELAY_ALLOW_PRIVATE=1`.
Connection, per-address, per-destination, packet-rate and bandwidth limits bound what one client and
the deployment as a whole can spend; an address may also open WebSockets only at
`RELAY_MAX_UPGRADES_PER_SEC`. `RELAY_TRUSTED_PROXIES` is the only reason a forwarded address
is believed. The relay must be able to reach the host's UDP port; over the internet the host needs
the same port forwarding a native client would. The threat model is in
[`DEPLOY.md`](DEPLOY.md#relay-threat-model).


## Landing page and SEO

The root page is an English landing page. Its play links open the English game
shell with `?lang=en`; resources are downloaded only after the player presses
**Start** on the game page, so no network work starts before that click. Direct
`game.html` visits behave the same way. Use the in-game fullscreen toolbar button
when desired. The existing game shell is available at `game.html` (including its
`?lang=`, `?data=` and other launch parameters).
The landing page is rendered at build time and delivered as complete HTML by
Nginx or the local server. All headings, links and FAQ answers work without
JavaScript; no Node server or client hydration is needed. The game is loaded
only on the play page.

Node >= 22 and npm are required to compile the pinned Tailwind CSS dependencies.
The normal `python3 tools/web/build.py` build includes the landing page. For a
landing-only build (no Rust compilation), run:

```sh
python3 tools/web/build_landing.py --site-url https://your-host.example/
```

Use the actual public URL, including a path prefix if hosted in a subdirectory.
`--site-url` (or `SITE_URL`) adds the canonical URL, Open Graph URL and
`sitemap.xml`; without a public URL these are deliberately omitted. A rebuild
without the URL removes any old sitemap. `robots.txt`, descriptive metadata and
SoftwareApplication structured data are emitted, and the play shell is marked
`noindex`. Submit the generated sitemap to the search engines after deployment.
Do not use the example hostname in production.

`packaging/web/landing.css` is the Tailwind entry point; `index.html` is its
explicit content source. CSS is compiled and served locally, without a CDN or
runtime CSS generator. The hero uses original generated Porsche 993 artwork, optimized as WebP; the
favicon is an original SVG. Neither asset was extracted from the game or copied
from the reference site. See `packaging/web/artwork.md` for the artwork prompt.
The landing is a player-facing entry point for a configured, ready-to-play host;
self-hosting and data preparation instructions remain in the documentation.

The landing source is GPL-3.0-or-later. The distribution includes `LICENSE` and
`THIRD-PARTY-NOTICES.txt` with Tailwind's MIT notice. When distributing the game
binary, provide the complete corresponding source for that exact build under
GPL, including local modifications and build instructions; a link to a different
upstream revision is insufficient. Original game assets are separate and are not
covered by the engine's GPL license.


### Loading screen

The non-commercial project landing lists keyboard, touch and standard browser
Gamepad API controller support. Gamepads require a button press or stick movement
before the browser exposes them; device/browser compatibility varies. Rumble follows
the browser's haptics support and works once the controller is active.

The play screen reuses the landing artwork and palette, with separate connection,
resource preparation and engine startup stages. Resource progress counts prepared
files, not downloaded bytes; connection and engine startup remain indeterminate.
The overlay stays up until the engine reports an intro, menu or race state.
Loading errors expose Retry or Reload and optional technical details. Track
preparation uses a compact panel over the game with only a status and percentage;
it keeps Retry and optional technical details on failure without discarding the
active game. The artwork, branding and startup steps appear only at first launch.
All loading UI strings are included in the six existing shell locales.
