# Game data

**English** | [Русский](../ru/game-data.md)

The engine contains no file of the original game. It reads the data of **your own copy** of Need for Speed:
Porsche Unleashed and never writes to it. In the browser build the data is prepared on the server and served
over HTTP; the visitor's browser only downloads it.

## What is needed

From the installed game — the folder that holds `nfs5.exe`:

| File / folder | Needed | What for |
|---|---|---|
| `GameData/` | yes | tracks, cars, physics, sounds |
| `FEData/` | yes | menus, fonts, texts, movies |
| `nfs5.exe` | no | the intro movies' decoder derives `madtables.bin` from it (it is never run); without compatible tables the intro is skipped |
| `fe.txt` | no | the original's settings file; without it the defaults are used |
| `nfs5.ini` | no | the Verok patch's settings, read with the same keys and defaults as in the patched original |
| `version.txt` | no | the game's version (`3.5`), used by the network version gate; the browser falls back to `3.5` when the file is absent |

## Preparing the web data

Run the packer from the repository root:

```sh
make web-data GAME_DIR=/path/to/game
make web-data GAME_DIR=/path/to/game MOVIE_EXE=/path/to/Porsche.exe
make web-data GAME_DIR=/path/to/game EXTRA_FILES=/path/to/extra.txt
```

The target copies only the files the browser build consumes, filters files no crate parses, validates each
candidate original executable and derives `madtables.bin` with the Rust parser. `MOVIE_EXE` overrides the
auto-detection; `EXTRA_FILES` adds a root file that is not inside `GAME_DIR`.

The result is the `web-data/` directory. It contains a `manifest.json` with case-insensitive paths, sizes,
groups and SHA-256 hashes, and content-addressed objects that the browser fetches and verifies independently.
Keep this directory outside Git: original game data and derived movie tables are never committed.

The server mounts the prepared directory read-only next to the application. The browser sees it under
`/data/` and exposes the game files to the engine under `/game`. Never serve the repository or the original
installation directory directly.

## `nfs5.ini`

The engine reads `nfs5.ini` from the game data with the same keys, defaults and effects as the patched
original; without the file the defaults apply. Section `[NFS5]`:

| Key (default) | Effect |
|---|---|
| `Language=English` | `English`, `French`, `German`, `Italian`, `Spanish`, `Swedish`, `PortBrzl` (any case); anything else is English |
| `NoMovies=0` | 1 skips the start-up movies |
| `Fog=1` | 0 turns the weather fog off |
| `Filtering=1` | 0 samples textures with nearest-neighbour instead of bilinear filtering |
| `CarControl=0` | 1 keeps the steering, throttle and brake live after a human car has finished the race |

`ThrashDriver`, `WindowedMode`, `SingleProcAffinity`, `CpuLimit` and `NoErrorReporting` belong to the
original's own start-up and are ignored.
