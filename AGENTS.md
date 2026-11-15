# Repository instructions

## Project and priorities

Wasm Unleashed is a Rust reimplementation of Need for Speed: Porsche Unleashed.
The delivery target is a self-contained browser game compiled to
`wasm32-unknown-unknown`. Desktop and mobile browsers are in scope. Native CLI
tools remain useful for asset preparation. Browser multiplayer joins a native
host at an IPv4 address through a WebSocket↔UDP relay; hosting and LAN discovery
remain native features because browsers cannot open UDP sockets.

Preserve the original game rules, physics, AI, file formats and save compatibility.
Keep platform changes separate from gameplay changes. Write code comments,
repository instructions and commit messages in English. Preserve the existing
English and Russian user documentation.

## Architecture

- `crates/open-unleashed`: application lifecycle, menus, race orchestration and
  browser startup in `src/web.rs`.
- `crates/nfs-platform`: platform filesystem, path predicates, clocks and scoped
  work. Browser files expose resources that JavaScript has already prepared.
- Other `crates/nfs-*` packages implement formats, assets, rendering, audio,
  input, physics and the original network protocol. `crates/nfs-net` also defines
  the browser relay framing (`src/relay.rs`); `tools/net-relay` is the separate
  Rust gateway that turns one WebSocket into one connected UDP flow.
- `packaging/web`: browser shell, HTTP asset source, virtual filesystem,
  IndexedDB persistence and Docker/Nginx configuration.
- `tools/web`: build, asset packaging, local serving and optional browser checks.

Use `nfs_platform::fs`, `nfs_platform::time` and `PathExt` in shared runtime code
instead of bypassing the browser adapters. Do not introduce blocking waits or
native thread creation into browser execution paths. GPU initialization and
network preparation must be asynchronous. Prepare resources before changing
game state; do not silently substitute empty data for required assets.

Keep WebGPU and WebGL2 rendering functional. WebGL2 uses dynamic uniform buffers
instead of object storage buffers. Surface copy capabilities must be checked;
effects that need readback may require an intermediate render target.

Browser game data is read-only under `/game`; writable progress belongs under
`/saves`. Preserve case-insensitive asset lookup and content-hash verification.
Keep save persistence independent from the asset cache. Music archives support
segmented loading: listing songs must not download the entire archive.

## Build and validation

Use Rust stable >= 1.95, Python >= 3.11 and Node >= 22. The wasm-bindgen CLI must
match the version in `Cargo.lock`. See `packaging/web/README.md` for setup,
resource preparation, hosting and known limitations.

Run checks appropriate to the change:

```sh
cargo +stable check --locked -p open-unleashed --target wasm32-unknown-unknown
cargo +stable test --locked -p nfs-formats -p nfs-net --lib -p nfs-relay
python3 -m unittest discover -s tools/web -p 'test_*.py'
node --test packaging/web/assets.test.mjs packaging/web/vfs.test.mjs packaging/web/i18n.test.mjs packaging/web/exit-game.test.mjs packaging/web/net.test.mjs
python3 tools/web/build.py
```

For local play, run `make serve` (it builds and starts the relay and serves
`dist/`) and open `http://127.0.0.1:8080`. `python3 tools/web/serve.py` without
`--relay` is the static-only server. The default directories are `dist/` and
`web-data/`.
Remote deployment needs HTTPS for browser APIs and CORS if assets use a separate
origin. Stop any local test server you started when the task is complete; do not
stop a server owned by the user or another task.

Keep browser gameplay testing basic unless explicitly requested otherwise.
Check startup and the behavior affected by the change; leave extended
playthroughs to the user. Do not repeat long game sessions after successful
checks without a specific regression to investigate. Report which browsers and
devices were actually tested; do not imply mobile or hardware WebGPU coverage
from a software-rendered Firefox check.

## Original data and generated files

Never commit original game data, executables, extracted tables, textures, models,
audio, videos or saves. This includes derived `madtables.bin` files. Resource
preparation must not modify the source installation. Keep generated builds,
private asset packages, browser profiles and screenshots in ignored directories
such as `dist/`, `web-data/` and `target/`.

Validate executable-derived movie tables before publishing them. A compatible
filename or version label alone does not establish that the tables are valid.
Keep original assets out of the Docker build context and the application image;
mount prepared server data separately.

New dependencies must be compatible with GPL-3.0-or-later. Before committing,
inspect staged changes and run `git diff --cached --check`. Include only source,
configuration and documentation intended for the change.
