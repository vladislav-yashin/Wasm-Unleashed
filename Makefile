# Local development and data preparation commands.
#
# The original movie tables are mandatory. The packer validates the game
# executables in GAME_DIR and uses the first compatible one.
#
# Usage:
#   make web-data GAME_DIR=/path/to/game
#   make web-data GAME_DIR=/path/to/game MOVIE_EXE=/path/to/PORSCHE.ICD

PYTHON ?= python3
CARGO ?= cargo
RUST_TOOLCHAIN ?= stable
WASM_TARGET ?= wasm32-unknown-unknown

GAME_DIR ?=
MOVIE_EXE ?=
WEB_DATA ?= web-data
EXTRA_FILES ?=

WEB_HOST ?= 127.0.0.1
WEB_PORT ?= 8080
WEB_BUILD_FLAGS ?=

WASM_BINDGEN_VERSION := $(shell awk '/^name = "wasm-bindgen"$$/{getline; gsub(/version = "|"/, ""); print; exit}' Cargo.lock)

.PHONY: help setup-web install-wasm-bindgen build-web web-data serve \
	test test-rust test-web check-wasm check-deploy ci fmt fmt-check \
	docker-up docker-down docker-logs clean clean-all

help:
	@printf '%s\n' \
		'Browser development:' \
		'  make setup-web                  install WASM target and wasm-bindgen CLI' \
		'  make build-web                  build dist/ (WEB_BUILD_FLAGS=--debug for a debug build)' \
		'  make web-data GAME_DIR=...      prepare web-data; movie executable auto-detected' \
		'  make serve                      serve dist/ and web-data on WEB_HOST:WEB_PORT' \
		'  make docker-up                  run the local compose stack' \
		'  make docker-down                stop the local compose stack' \
		'  make docker-logs                follow local container logs' \
		'  make clean                      remove dist/' \
		'  make clean-all                  remove dist/ and the Rust target/' \
		'' \
		'Checks:' \
		'  make test                       Rust and web tests' \
		'  make test-rust                  cargo tests for nfs-formats and nfs-net' \
		'  make test-web                   Python and Node web tests' \
		'  make check-wasm                 cargo check for wasm32' \
		'  make check-deploy               validate deployment scripts and compose' \
		'  make ci                         all checks above' \
		'  make fmt                        format Rust code' \
		'  make fmt-check                  check Rust formatting' \
		'' \
		'Variables:' \
		'  GAME_DIR       original installation root containing GameData/ and FEData/' \
		'  MOVIE_EXE      optional PE image override; auto-detected by compatibility' \
		'  WEB_DATA       data directory (default: web-data)' \
		'  EXTRA_FILES    additional root files for web-data, space-separated' \
		'  WEB_HOST       local server host (default: 127.0.0.1)' \
		'  WEB_PORT       local server port (default: 8080)' \
		'  PYTHON         Python interpreter (default: python3)' \
		'  RUST_TOOLCHAIN Rust toolchain (default: stable)'

setup-web:
	rustup target add $(WASM_TARGET) --toolchain $(RUST_TOOLCHAIN)
	$(MAKE) install-wasm-bindgen

install-wasm-bindgen:
	@test -n "$(WASM_BINDGEN_VERSION)" || { echo "wasm-bindgen version not found in Cargo.lock" >&2; exit 2; }
	$(CARGO) +$(RUST_TOOLCHAIN) install wasm-bindgen-cli --version $(WASM_BINDGEN_VERSION) --locked

build-web:
	$(PYTHON) tools/web/build.py $(WEB_BUILD_FLAGS)

web-data:
	@if [ -z "$(GAME_DIR)" ]; then \
		echo "GAME_DIR is required. Run 'make help' for usage." >&2; \
		exit 2; \
	fi
	@if [ ! -d "$(GAME_DIR)/GameData" ] || [ ! -d "$(GAME_DIR)/FEData" ]; then \
		echo "GAME_DIR must contain GameData/ and FEData/: $(GAME_DIR)" >&2; \
		exit 2; \
	fi
	@if [ -n "$(MOVIE_EXE)" ] && [ ! -f "$(MOVIE_EXE)" ]; then \
		echo "MOVIE_EXE not found: $(MOVIE_EXE)" >&2; \
		exit 2; \
	fi
	@if ! command -v "$(PYTHON)" >/dev/null 2>&1; then \
		echo "Python interpreter not found: $(PYTHON)" >&2; \
		exit 2; \
	fi
	@if ! command -v cargo >/dev/null 2>&1; then \
		echo "cargo is required to extract movie tables; install Rust stable." >&2; \
		exit 2; \
	fi
	$(PYTHON) tools/web/pack.py "$(GAME_DIR)" "$(WEB_DATA)" --from-installation \
		$(if $(MOVIE_EXE),--movie-exe "$(MOVIE_EXE)") \
		$(foreach file,$(EXTRA_FILES),--extra-file "$(file)")
	@$(PYTHON) -c "import json, sys; files=json.load(open('$(WEB_DATA)/manifest.json'))['files']; sys.exit(0 if 'madtables.bin' in files else 'madtables.bin missing from web-data')"

serve:
	$(PYTHON) tools/web/serve.py --host "$(WEB_HOST)" --port "$(WEB_PORT)" --data "$(WEB_DATA)"

test: test-rust test-web

test-rust:
	$(CARGO) +$(RUST_TOOLCHAIN) test --locked -p nfs-formats -p nfs-net --lib

test-web:
	$(PYTHON) -m unittest discover -s tools/web -p 'test_*.py'
	node --test packaging/web/assets.test.mjs packaging/web/vfs.test.mjs

check-wasm:
	$(CARGO) +$(RUST_TOOLCHAIN) check --locked -p open-unleashed --target $(WASM_TARGET)

check-deploy:
	$(PYTHON) packaging/web/deploy/generate-setup.py --check
	bash -n packaging/web/deploy/remote-deploy.sh
	bash -n packaging/web/deploy/setup-server.sh
	OPEN_UNLEASHED_IMAGE=ghcr.io/example/open-unleashed@sha256:0000000000000000000000000000000000000000000000000000000000000000 \
		docker compose -f packaging/web/compose.deploy.yaml config >/dev/null
	$(MAKE) -n web-data GAME_DIR=/tmp/game >/dev/null

ci: check-wasm check-deploy test-rust test-web

fmt:
	$(CARGO) +$(RUST_TOOLCHAIN) fmt --all

fmt-check:
	$(CARGO) +$(RUST_TOOLCHAIN) fmt --all -- --check

docker-up:
	NFS_WEB_DATA="$(abspath $(WEB_DATA))" docker compose -f packaging/web/compose.yaml up --build

docker-down:
	NFS_WEB_DATA="$(abspath $(WEB_DATA))" docker compose -f packaging/web/compose.yaml down

docker-logs:
	NFS_WEB_DATA="$(abspath $(WEB_DATA))" docker compose -f packaging/web/compose.yaml logs --tail=100 -f

clean:
	rm -rf dist

clean-all: clean
	$(CARGO) +$(RUST_TOOLCHAIN) clean
