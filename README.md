<h1 align="center">Wasm Unleashed</h1>

<p align="center"><b>English</b> | <a href="README.ru.md">Русский</a></p>

An unofficial, open-source browser reimplementation of the engine of **Need for Speed: Porsche Unleashed**
(PC, 2000). It runs in modern desktop and mobile browsers from WebAssembly: no installation, no Wine, no
emulation and no original executable. Physics, opponents and traffic, the Evolution career, Factory Driver and
the original menus play as in the original. Game data is served over HTTP, and progress is stored in the
browser's IndexedDB.

An independent implementation written from scratch in Rust (wgpu with WebGPU and WebGL2) that reproduces the
original's behaviour.

> **You need your own copy of the game.** No file of the original game is in this repository or in its
> builds. The project is not affiliated with Electronic Arts, Eden Games or Porsche AG; "Need for Speed" and
> "Porsche" are trademarks of their owners and are named here only to say which game this engine runs.

## Play in the browser

The engine compiles to `wasm32-unknown-unknown` and runs entirely in the browser. Open a hosted build, or
prepare and serve your own copy: [browser build and hosting](packaging/web/README.md). Multiplayer is planned
for a later stage and is not available in the browser build yet.

## Documentation

| | English | Русский |
|---|---|---|
| First start, saves and troubleshooting | [docs/en/install.md](docs/en/install.md) | [docs/ru/install.md](docs/ru/install.md) |
| Preparing the game data | [docs/en/game-data.md](docs/en/game-data.md) | [docs/ru/game-data.md](docs/ru/game-data.md) |
| Controls | [docs/en/controls.md](docs/en/controls.md) | [docs/ru/controls.md](docs/ru/controls.md) |
| Building and hosting | [docs/en/building.md](docs/en/building.md) | [docs/ru/building.md](docs/ru/building.md) |

## Quick start

To host the game yourself:

1. Install the wasm target and the matching wasm-bindgen CLI with `make setup-web`.
2. Prepare the original game data with `make web-data GAME_DIR=/path/to/game`.
3. Build the browser distribution with `make build-web`.
4. Start a local server with `make serve` and open `http://127.0.0.1:8080`.

Details and hosting requirements: [browser build](packaging/web/README.md).

## Licence

GNU General Public License v3.0 or later ([LICENSE](LICENSE)). Third-party components keep their own licences.

## Contributing

Bug reports and pull requests: [CONTRIBUTING.md](CONTRIBUTING.md).

## Acknowledgements

Wasm Unleashed is a fork of the **OpenUnleashed** project. Thanks to its author, fussraider, and to everyone
who contributed to the original implementation.

The community patch 1.0.6 by Verok, whose run-time fixes to the original executable this engine reproduces
where they change the game's behaviour.


The web build serves a search-indexable landing page at `/` and the game at
`game.html`. Tailwind CSS is compiled locally. Pass the actual public URL with
`python3 tools/web/build.py --site-url https://your-host.example/` to generate
canonical metadata and a sitemap. See [landing page and SEO](packaging/web/README.md#landing-page-and-seo).
