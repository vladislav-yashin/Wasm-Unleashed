# Contributing

**English** | [Русский](CONTRIBUTING.ru.md)

## Reporting a problem

Open an issue with the matching template and include:

- your browser and version, device and GPU model, and the build URL or commit you run;
- what you did, what you expected and what happened instead;
- errors from the browser's developer console, and the exported saves when the problem concerns progress
  ([first start](docs/en/install.md)).

**Never attach files of the original game** — no `GameData` / `FEData` files, no `nfs5.exe`, no extracted textures,
sounds or models. A screenshot or a short video is fine. Issues may be written in English or Russian.

## Pull requests

- The aim is to play like the original. A change to game rules, physics, AI or the network protocol has to match what
  the original game does; say in the description how you checked it.
- `make ci` must pass: the wasm check, the deployment validation, the Rust tests and the Python/Node web
  tests. `make fmt-check` covers formatting (the tests that need the game data skip themselves without it).
- Nothing of the original game goes into the repository, not even extracted or converted data.
- New dependencies must have a licence compatible with GPL-3.0 (MIT, Apache-2.0, BSD, Zlib, ISC, MPL-2.0, LGPL, CC0).
- Your contribution is licensed under the GNU GPL v3.0 or later, like the rest of the project.

Pull requests are merged into `main` as they are, so you stay the author of your commits.
