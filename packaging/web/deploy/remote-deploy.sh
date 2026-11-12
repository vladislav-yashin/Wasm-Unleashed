#!/usr/bin/env bash
# Remote deployment helper for the Wasm Unleashed web build.
#
# Install as /usr/local/bin/open-unleashed-deploy and restrict the CI SSH key to
# this command, for example:
#
#   restrict,command="/usr/local/bin/open-unleashed-deploy" ssh-ed25519 AAAA...
#
# The command line is "deploy <image>@sha256:<digest>". The image must belong to
# OPEN_UNLEASHED_IMAGE_REPO; arbitrary commands are rejected. An administrator
# may run the script directly with the image reference as the only argument.

set -euo pipefail
umask 077

log() {
    echo "deploy: $*"
}

die() {
    echo "deploy: $*" >&2
    exit 1
}

COMPOSE_FILE="${OPEN_UNLEASHED_COMPOSE_FILE:-/opt/open-unleashed/compose.yaml}"
ENV_FILE="${OPEN_UNLEASHED_ENV_FILE:-/etc/open-unleashed/deploy.env}"
STATE_DIR="${OPEN_UNLEASHED_STATE_DIR:-/var/lib/open-unleashed}"
IMAGE_REPO="${OPEN_UNLEASHED_IMAGE_REPO:-ghcr.io/vladislav-yashin/wasm-unleashed}"
HEALTH_TIMEOUT="${OPEN_UNLEASHED_HEALTH_TIMEOUT:-90}"
SETTLE_SECONDS="${OPEN_UNLEASHED_SETTLE_SECONDS:-10}"
DRAIN_SECONDS="${OPEN_UNLEASHED_DRAIN_SECONDS:-10}"
STOP_GRACE_SECONDS="${OPEN_UNLEASHED_STOP_GRACE_SECONDS:-30}"

if [[ -f "$ENV_FILE" ]]; then
    # The file is installed root-owned. It contains paths and ports, not secrets.
    set -a
    # shellcheck disable=SC1090
    source "$ENV_FILE"
    set +a
fi

COMPOSE_FILE="${OPEN_UNLEASHED_COMPOSE_FILE:-$COMPOSE_FILE}"
STATE_DIR="${OPEN_UNLEASHED_STATE_DIR:-$STATE_DIR}"
IMAGE_REPO="${OPEN_UNLEASHED_IMAGE_REPO:-$IMAGE_REPO}"
IMAGE_REPO="${IMAGE_REPO,,}"
HEALTH_TIMEOUT="${OPEN_UNLEASHED_HEALTH_TIMEOUT:-$HEALTH_TIMEOUT}"
SETTLE_SECONDS="${OPEN_UNLEASHED_SETTLE_SECONDS:-$SETTLE_SECONDS}"
DRAIN_SECONDS="${OPEN_UNLEASHED_DRAIN_SECONDS:-$DRAIN_SECONDS}"
STOP_GRACE_SECONDS="${OPEN_UNLEASHED_STOP_GRACE_SECONDS:-$STOP_GRACE_SECONDS}"
PUBLIC_HEALTH_URL="${DEPLOY_HEALTH_URL:-}"

[[ -f "$COMPOSE_FILE" ]] || die "compose file not found: $COMPOSE_FILE"
[[ -d "$STATE_DIR" ]] || die "state directory not found: $STATE_DIR (run deploy/setup-server.sh)"
command -v docker >/dev/null 2>&1 || die "docker is not installed"
docker compose version >/dev/null 2>&1 || die "docker compose plugin is not available"
command -v flock >/dev/null 2>&1 || die "flock is required"
if [[ -n "$PUBLIC_HEALTH_URL" ]]; then
    command -v curl >/dev/null 2>&1 || die "curl is required when DEPLOY_HEALTH_URL is set"
fi

image=""
if [[ -n "${SSH_ORIGINAL_COMMAND:-}" ]]; then
    [[ "$SSH_ORIGINAL_COMMAND" != *$'\n'* && "$SSH_ORIGINAL_COMMAND" != *$'\r'* ]] \
        || die "SSH command must be a single line"
    read -r verb image extra <<<"$SSH_ORIGINAL_COMMAND"
    [[ -z "${extra:-}" ]] || die "unexpected arguments in SSH command"
    [[ "$verb" == "deploy" && -n "$image" ]] || die "expected SSH command: deploy <image>"
elif [[ $# -eq 1 ]]; then
    image="$1"
else
    die "usage: open-unleashed-deploy <image>"
fi

if [[ "$image" != "$IMAGE_REPO@"* ]]; then
    die "image must come from $IMAGE_REPO"
fi
digest="${image#"$IMAGE_REPO@"}"
[[ "$digest" =~ ^sha256:[0-9a-f]{64}$ ]] || die "image must be pinned by sha256 digest"

exec 9>"$STATE_DIR/deploy.lock"
flock -n 9 || die "another deployment is already running"

COMPOSE=(docker compose --project-directory "$(dirname "$COMPOSE_FILE")" --file "$COMPOSE_FILE")
if [[ -f "$ENV_FILE" ]]; then
    COMPOSE+=(--env-file "$ENV_FILE")
fi

export OPEN_UNLEASHED_IMAGE="$image"

data_dir="${NFS_WEB_DATA:-/srv/open-unleashed/web-data}"
[[ -d "$data_dir" ]] || die "web data directory not found: $data_dir"
[[ -f "$data_dir/manifest.json" ]] || die "web data is not prepared: $data_dir/manifest.json is missing"

active=""
if [[ -f "$STATE_DIR/active" ]]; then
    active="$(cat "$STATE_DIR/active")"
    [[ "$active" == "blue" || "$active" == "green" ]] || die "invalid active slot in $STATE_DIR/active"
fi

running="$("${COMPOSE[@]}" ps --status running --services 2>/dev/null || true)"
blue_running=0
green_running=0
if grep -qx "blue" <<<"$running"; then
    blue_running=1
fi
if grep -qx "green" <<<"$running"; then
    green_running=1
fi

if [[ -n "$active" ]]; then
    if [[ $blue_running -eq 1 && $green_running -eq 1 ]]; then
        die "both blue and green are running while state says $active; resolve the stale container manually"
    fi
    if [[ "$active" == "blue" && $blue_running -eq 0 ]] || [[ "$active" == "green" && $green_running -eq 0 ]]; then
        log "state says $active but that container is not running; recovering the active slot"
        if [[ "$active" == "blue" && $green_running -eq 1 ]] || [[ "$active" == "green" && $blue_running -eq 1 ]]; then
            if [[ $blue_running -eq 1 ]]; then
                active="blue"
            else
                active="green"
            fi
        else
            active=""
        fi
    fi
else
    if [[ $blue_running -eq 1 && $green_running -eq 1 ]]; then
        die "both blue and green are running and no state file exists; stop one container first"
    elif [[ $blue_running -eq 1 ]]; then
        active="blue"
    elif [[ $green_running -eq 1 ]]; then
        active="green"
    fi
fi

if [[ "$active" == "blue" ]]; then
    target="green"
elif [[ "$active" == "green" ]]; then
    target="blue"
else
    target="blue"
fi

log "active=${active:-none} target=$target image=$image"

new_container=""
old_container=""
new_started=0
old_drained=0
committed=0

cleanup() {
    local code=$?
    if [[ $committed -eq 0 ]]; then
        if [[ $old_drained -eq 1 && -n "$old_container" ]]; then
            docker exec "$old_container" rm -f /tmp/draining >/dev/null 2>&1 || true
            sleep "$SETTLE_SECONDS"
        fi
        if [[ $new_started -eq 1 && -n "$new_container" ]]; then
            # Let Caddy remove the new upstream before deleting it. If the old
            # upstream is healthy it takes over again.
            docker exec "$new_container" touch /tmp/draining >/dev/null 2>&1 || true
            sleep "$DRAIN_SECONDS"
            docker stop --time "$STOP_GRACE_SECONDS" "$new_container" >/dev/null 2>&1 || true
            docker rm --force "$new_container" >/dev/null 2>&1 || true
        fi
        if [[ -n "$active" ]]; then
            "${COMPOSE[@]}" rm --stop --force "$target" >/dev/null 2>&1 || true
        fi
    fi
    exit "$code"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

log "removing any previous $target container"
"${COMPOSE[@]}" rm --stop --force "$target" >/dev/null 2>&1 || true

log "pulling $image"
"${COMPOSE[@]}" pull "$target" >/dev/null

log "starting $target"
"${COMPOSE[@]}" up --detach --no-deps --force-recreate "$target" >/dev/null
new_container="$("${COMPOSE[@]}" ps --quiet "$target")"
[[ -n "$new_container" ]] || die "could not find the new $target container"
new_started=1

wait_healthy() {
    local deadline=$((SECONDS + HEALTH_TIMEOUT))
    while (( SECONDS < deadline )); do
        local status
        status="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$new_container" 2>/dev/null || echo missing)"
        case "$status" in
            healthy)
                return 0
                ;;
            unhealthy|exited|dead|missing)
                log "new container is $status"
                return 1
                ;;
        esac
        sleep 2
    done
    log "healthcheck did not pass within ${HEALTH_TIMEOUT}s"
    return 1
}

log "waiting for $target to become healthy"
wait_healthy || die "new $target container did not become healthy"
docker exec "$new_container" wget -q -T 3 -O /dev/null http://127.0.0.1:8080/ \
    || die "new $target container does not serve index.html"

if [[ -n "$active" ]]; then
    old_container="$("${COMPOSE[@]}" ps --quiet "$active")"
    [[ -n "$old_container" ]] || die "active $active container is not running"
fi

probe_public() {
    [[ -n "$PUBLIC_HEALTH_URL" ]] || return 0
    local deadline=$((SECONDS + 30))
    while (( SECONDS < deadline )); do
        if curl --fail --silent --show-error --max-time 5 "$PUBLIC_HEALTH_URL" >/dev/null 2>&1; then
            return 0
        fi
        sleep 2
    done
    return 1
}

# Wait for Caddy's active health checker before draining upstreams.
sleep "$SETTLE_SECONDS"

if [[ "$target" == "green" ]]; then
    # Blue is the first upstream, so Caddy keeps all traffic there until it is
    # drained. Only then does it move to green.
    if [[ -n "$old_container" ]]; then
        log "draining blue"
        docker exec "$old_container" touch /tmp/draining >/dev/null \
            || die "could not put the active blue container into draining mode"
        old_drained=1
        sleep "$DRAIN_SECONDS"
    fi
    probe_public || die "public health check failed after switching to green"
else
    # Blue is the first upstream. Caddy should have switched to it as soon as
    # it became healthy. Verify it before touching the old green container.
    probe_public || die "public health check failed after switching to blue"
fi

# Write the state before stopping the old container. If this fails, the cleanup
# handler restores the old upstream and removes the new one.
if [[ -f "$STATE_DIR/image" ]]; then
    cp "$STATE_DIR/image" "$STATE_DIR/image.previous"
fi
printf '%s\n' "$target" > "$STATE_DIR/active.new"
printf '%s\n' "$image" > "$STATE_DIR/image.new"
mv "$STATE_DIR/active.new" "$STATE_DIR/active"
mv "$STATE_DIR/image.new" "$STATE_DIR/image"
committed=1

if [[ -n "$old_container" ]]; then
    if [[ "$target" == "blue" ]]; then
        log "draining green"
        docker exec "$old_container" touch /tmp/draining >/dev/null 2>&1 || true
        old_drained=1
        sleep "$DRAIN_SECONDS"
    fi
    log "stopping $active"
    docker stop --time "$STOP_GRACE_SECONDS" "$old_container" >/dev/null 2>&1 || true
    docker rm --force "$old_container" >/dev/null 2>&1 \
        || die "could not remove the old $active container; remove it manually"
    "${COMPOSE[@]}" rm --stop --force "$active" >/dev/null 2>&1 || true
fi

log "deployed $image to $target"
