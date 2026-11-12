# Browser build

The engine now compiles to `wasm32-unknown-unknown`. Original resources are served
via HTTP and presented to existing parsers through a preloaded virtual filesystem.
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
python3 tools/web/pack.py /path/to/original/game web-data
# Optional: include movie tables and original front-end configuration:
python3 tools/web/pack.py /path/to/game web-data --movie-exe /path/to/original/PORSCHE.ICD --extra-file /path/to/original/fe.txt
# Start from the repository root:
docker compose -f packaging/web/compose.yaml up --build
```

Open http://localhost:8080. For remote devices, put the service behind HTTPS;
secure context browser APIs require it. Localhost is suitable for development.
For local development without Docker, run `python3 tools/web/serve.py` and open
http://127.0.0.1:8080. For production, put `dist/` behind a static HTTP server and
mount the prepared `web-data/` directory at `/data/`. Never serve the repository or original
installation directly. `?data=https://example.test/data/` changes the resource
base URL; a different origin must supply CORS headers.

`build.py --debug` produces a larger development build. The build checks that the
wasm-bindgen CLI version matches Cargo.lock. `smoke.html` initializes the renderer
without original game resources; it is a diagnostic page, not a gameplay test.

## Loading and saves

The manifest contains case-insensitive paths, sizes, groups and SHA-256 hashes.
Content-addressed objects are independently fetched, verified and cached.
`--movie-exe` extracts the small `madtables.bin` using the existing Rust parser;
the executable is not published. Without these tables original videos are skipped. Root
installation files are included because configuration and movie-table loaders
use them. Original data is not included in this repository or the web build.

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
name entry is provided by the HTML field; tap the corresponding game field first.
Browser audio uses CPAL's Web Audio output, with asynchronous music feeding and
resume on user interaction. It does not yet use a dedicated AudioWorklet mixer.
Wheel force feedback and network sessions are unavailable in the web version.

## Validation and remaining acceptance work

```sh
cargo +stable check -p open-unleashed --target wasm32-unknown-unknown --locked
cargo +stable test -p nfs-formats -p nfs-net --lib
python3 -m unittest discover -s tools/web -p 'test_*.py'
node --test packaging/web/assets.test.mjs packaging/web/vfs.test.mjs
```

Remaining acceptance: a complete quick race, career and mission, audio continuity,
mobile memory/performance, context loss/recovery and persistent save round trips
on real browsers with full original resources. Browser photos download the current canvas resolution; native high-resolution
photo scales do not apply to this path.
Future multiplayer needs a WebSocket transport and a UDP gateway; the existing
wire protocol remains separate from browser asset delivery.
