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
EXE := $(if $(filter Windows_NT,$(OS)),.exe,)
RELAY_BINARY ?= target/debug/nfs-relay$(EXE)

DEPLOY_SSH_HOST ?=
DEPLOY_SSH_PORT ?= 22
DEPLOY_SSH_USER ?=
DEPLOY_HELPER ?= /usr/local/bin/open-unleashed-deploy
DEPLOY_COMPOSE ?= /opt/open-unleashed/compose.yaml

WASM_BINDGEN_VERSION := $(shell awk '/^name = "wasm-bindgen"$$/{getline; gsub(/version = "|"/, ""); print; exit}' Cargo.lock)

.PHONY: help setup-web install-wasm-bindgen build-web web-data serve relay-build relay \
	test test-rust test-web check-wasm check-deploy ci fmt fmt-check \
	deploy-sync \
	docker-up docker-down docker-logs clean clean-all

help:
	@printf '%s\n' \
		'Browser development:' \
		'  make setup-web                  install WASM target and wasm-bindgen CLI' \
		'  make build-web                  build dist/ (WEB_BUILD_FLAGS=--debug for a debug build)' \
		'  make web-data GAME_DIR=...      prepare web-data; movie executable auto-detected' \
		'  make serve                      serve dist/ and web-data, starting the relay too' \
		'  make relay-build                build the relay binary used by serve' \
		'  make relay                      run a standalone UDP relay on 127.0.0.1:8090' \
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
		'Deployment:' \
		'  make deploy-sync                sync the compose file and helper to DEPLOY_SSH_HOST' \
		'' \
		'Variables:' \
		'  GAME_DIR       original installation root containing GameData/ and FEData/' \
		'  MOVIE_EXE      optional PE image override; auto-detected by compatibility' \
		'  WEB_DATA       data directory (default: web-data)' \
		'  EXTRA_FILES    additional root files for web-data, space-separated' \
		'  WEB_HOST       local server host (default: 127.0.0.1)' \
		'  WEB_PORT       local server port (default: 8080)' \
		'  PYTHON         Python interpreter (default: python3)' \
		'  RUST_TOOLCHAIN Rust toolchain (default: stable)' \
		'  DEPLOY_SSH_HOST  deploy host for deploy-sync (required there)' \
		'  DEPLOY_SSH_PORT  deploy SSH port (default: 22)' \
		'  DEPLOY_SSH_USER  optional SSH login for deploy-sync (default: current user)'

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

relay-build:
	$(CARGO) +$(RUST_TOOLCHAIN) build --locked -p nfs-relay

serve: relay-build
	$(PYTHON) tools/web/serve.py --host "$(WEB_HOST)" --port "$(WEB_PORT)" --data "$(WEB_DATA)" \
		--relay --relay-binary "$(RELAY_BINARY)"

relay:
	$(CARGO) +$(RUST_TOOLCHAIN) run --locked -p nfs-relay -- --listen 127.0.0.1:8090 --allow-loopback --allow-private

test: test-rust test-web

test-rust:
	$(CARGO) +$(RUST_TOOLCHAIN) test --locked -p nfs-formats -p nfs-net --lib -p nfs-relay

test-web:
	$(PYTHON) -m unittest discover -s tools/web -p 'test_*.py'
	node --test packaging/web/assets.test.mjs packaging/web/vfs.test.mjs packaging/web/i18n.test.mjs packaging/web/controls.test.mjs packaging/web/fullscreen.test.mjs packaging/web/render-scale.test.mjs packaging/web/unlock-all.test.mjs packaging/web/startup-assets.test.mjs packaging/web/net.test.mjs

check-wasm:
	$(CARGO) +$(RUST_TOOLCHAIN) check --locked -p open-unleashed --target $(WASM_TARGET)

check-deploy:
	$(PYTHON) packaging/web/deploy/generate-setup.py --check
	bash -n packaging/web/deploy/remote-deploy.sh
	bash -n packaging/web/deploy/setup-server.sh
	OPEN_UNLEASHED_IMAGE=ghcr.io/example/open-unleashed@sha256:0000000000000000000000000000000000000000000000000000000000000000 \
	OPEN_UNLEASHED_RELAY_IMAGE=ghcr.io/example/open-unleashed-relay@sha256:0000000000000000000000000000000000000000000000000000000000000000 \
		docker compose -f packaging/web/compose.deploy.yaml config >/dev/null
	$(MAKE) -n web-data GAME_DIR=/tmp/game >/dev/null

ci: check-wasm check-deploy test-rust test-web

# Install the deployment inputs on a prepared server without re-running the whole
# setup script: the compose file and the restricted deploy helper are copied and
# checked against the local sources. deploy.env, the SSH key and authorized_keys
# are left untouched. Requires sudo on the server; DEPLOY_SSH_USER defaults to
# the current SSH user and the port to 22.
deploy-sync:
	@if [ -z "$(DEPLOY_SSH_HOST)" ]; then \
		echo "DEPLOY_SSH_HOST is required. Run 'make help' for usage." >&2; \
		exit 2; \
	fi
	@$(PYTHON) packaging/web/deploy/generate-setup.py --check >/dev/null
	@set -e; \
	port="$(or $(DEPLOY_SSH_PORT),22)"; \
	target="$(if $(DEPLOY_SSH_USER),$(DEPLOY_SSH_USER)@,)$(DEPLOY_SSH_HOST)"; \
	stage="$$(ssh -p "$$port" "$$target" 'mktemp -d')"; \
	cleanup() { ssh -p "$$port" "$$target" rm -rf "$$stage" >/dev/null 2>&1 || true; }; \
	trap cleanup EXIT INT TERM; \
	scp -q -P "$$port" packaging/web/deploy/remote-deploy.sh "$$target:$$stage/remote-deploy.sh"; \
	scp -q -P "$$port" packaging/web/compose.deploy.yaml "$$target:$$stage/compose.deploy.yaml"; \
	ssh -p "$$port" "$$target" "set -e; \
		sudo cp -a $(DEPLOY_HELPER) $(DEPLOY_HELPER).bak 2>/dev/null || true; \
		sudo install -m 0755 -o root -g root '$$stage/remote-deploy.sh' $(DEPLOY_HELPER); \
		sudo install -m 0644 -o root -g root '$$stage/compose.deploy.yaml' $(DEPLOY_COMPOSE)"; \
	helper_sum="$$(sha256sum packaging/web/deploy/remote-deploy.sh | cut -d' ' -f1)"; \
	compose_sum="$$(sha256sum packaging/web/compose.deploy.yaml | cut -d' ' -f1)"; \
	remote_sum="$$(ssh -p "$$port" "$$target" "sha256sum $(DEPLOY_HELPER) $(DEPLOY_COMPOSE)")"; \
	printf '%s\n' "$$remote_sum" | grep -q "^$$helper_sum " \
		|| { echo "$(DEPLOY_HELPER) on $$target does not match remote-deploy.sh" >&2; exit 1; }; \
	printf '%s\n' "$$remote_sum" | grep -q "^$$compose_sum " \
		|| { echo "$(DEPLOY_COMPOSE) on $$target does not match compose.deploy.yaml" >&2; exit 1; }; \
	printf 'synced %s and %s on %s\n' "$(DEPLOY_HELPER)" "$(DEPLOY_COMPOSE)" "$$target"

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
