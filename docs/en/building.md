# Building and hosting

**English** | [Русский](../ru/building.md)

## Requirements

- Rust 1.95 or newer with the `wasm32-unknown-unknown` target (`rustup`);
- the `wasm-bindgen` CLI matching the version in `Cargo.lock`;
- Python 3.11 or newer;
- Node 22 or newer for the JavaScript tests;
- Docker with the Compose plugin, optional, for the container image and the deployment checks;
- the game data as described in [game data](game-data.md).

## Build and run locally

```sh
make setup-web                 # add the wasm target and install the matching wasm-bindgen CLI
make web-data GAME_DIR=/path/to/game
make build-web                 # build dist/ (WEB_BUILD_FLAGS=--debug for a debug build)
make serve                     # serve dist/ and web-data/ on http://127.0.0.1:8080
```

Without `make`, the same steps are:

```sh
rustup target add wasm32-unknown-unknown --toolchain stable
cargo +stable install wasm-bindgen-cli --version <version from Cargo.lock> --locked
python3 tools/web/build.py
python3 tools/web/serve.py --data web-data
```

`tools/web/build.py` checks that the installed wasm-bindgen CLI matches `Cargo.lock`, builds the
`wasm32-unknown-unknown` target and stages the browser shell into `dist/`. `make web-data` prepares the
read-only game assets in `web-data/`; `make serve` serves both directories and sets the correct MIME types
for WebAssembly modules.

For a container-based local run:

```sh
docker compose -f packaging/web/compose.yaml up --build
```

Open `http://127.0.0.1:8080`. The default directories are `dist/` and `web-data/`.

## Tests and checks

```sh
make test                      # Rust and web tests
make check-wasm                # cargo check for wasm32
make check-deploy              # deployment scripts and compose validation
make ci                        # all checks above
make fmt-check                 # Rust formatting
```

The equivalent commands:

```sh
cargo +stable check --locked -p open-unleashed --target wasm32-unknown-unknown
cargo +stable test --locked -p nfs-formats -p nfs-net --lib
python3 -m unittest discover -s tools/web -p 'test_*.py'
node --test packaging/web/assets.test.mjs packaging/web/vfs.test.mjs
```

Tests that read the original data skip themselves when the game is not found.

## Distribution and deployment

`make build-web` writes the browser distribution to `dist/`: `index.html`, the browser modules and the
wasm-bindgen package. It never contains original game data. The prepared `web-data/` directory is served
separately and mounted read-only; a production deployment, the Caddy/HTTPS setup and the blue-green release
flow are described in [deployment](../../packaging/web/DEPLOY.md).

## Code layout

| Crate | Role |
|---|---|
| `nfs-formats` | the original's file formats |
| `nfs-assets` | game-data location, tracks, cars, textures |
| `nfs-physics` | vehicle model, collisions |
| `nfs-game` | race logic, AI, traffic, police, career, front-end runtime |
| `nfs-renderer` | wgpu renderer reproducing the original's look on WebGPU and WebGL2 |
| `nfs-audio`, `nfs-input`, `nfs-net` | sound, controls and force feedback, the original network protocol |
| `open-unleashed` | the game application and browser startup |
| `packaging/web` | browser shell, asset source, virtual filesystem and persistence |
| `tools/web` | build, asset packaging, local serving and optional browser checks |
