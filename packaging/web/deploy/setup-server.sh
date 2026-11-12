#!/usr/bin/env bash
# One-time server setup for the OpenUnleashed web deployment.
#
# This generated script is self-contained: it embeds the production compose
# file, the restricted remote deploy helper, and the default deploy.env.
# Copy only this file to the server and run it as root:
#
#   sudo OPEN_UNLEASHED_SSH_HOST=your.host.example bash setup-server.sh
#
# The embedded copies are checked against packaging/web/compose.deploy.yaml,
# packaging/web/deploy/remote-deploy.sh and
# packaging/web/deploy/deploy.env.example in CI.

set -euo pipefail
umask 077

if [[ $EUID -ne 0 ]]; then
    echo "Run this script as root: sudo bash setup-server.sh" >&2
    exit 1
fi

DEPLOY_USER="open-unleashed-deployer"
DEPLOY_SCRIPT="/usr/local/bin/open-unleashed-deploy"
COMPOSE_DIR="/opt/open-unleashed"
COMPOSE_FILE="$COMPOSE_DIR/compose.yaml"
ENV_DIR="/etc/open-unleashed"
ENV_FILE="$ENV_DIR/deploy.env"
STATE_DIR="/var/lib/open-unleashed"
DATA_DIR="/srv/open-unleashed/web-data"
SSH_PORT="${OPEN_UNLEASHED_SSH_PORT:-22}"
SSH_HOST="${OPEN_UNLEASHED_SSH_HOST:-}"

command -v docker >/dev/null 2>&1 || { echo "docker is not installed" >&2; exit 1; }
docker compose version >/dev/null 2>&1 || { echo "docker compose plugin is not installed" >&2; exit 1; }
getent group docker >/dev/null 2>&1 || { echo "docker group does not exist" >&2; exit 1; }
command -v flock >/dev/null 2>&1 || { echo "flock is not installed (util-linux)" >&2; exit 1; }

if id "$DEPLOY_USER" >/dev/null 2>&1; then
    echo "User $DEPLOY_USER already exists."
else
    useradd --create-home --shell /bin/bash "$DEPLOY_USER"
    echo "Created user $DEPLOY_USER."
fi

usermod --append --groups docker "$DEPLOY_USER"

install -d -m 0755 "$COMPOSE_DIR" "$ENV_DIR"
install -d -m 0750 -o "$DEPLOY_USER" -g "$DEPLOY_USER" "$STATE_DIR"
[[ -d /srv/open-unleashed ]] || install -d -m 0755 /srv/open-unleashed
[[ -d "$DATA_DIR" ]] || install -d -m 0755 "$DATA_DIR"

cat > "$COMPOSE_FILE" <<'OPEN_UNLEASHED_COMPOSE_EOF'
# Production compose file for blue-green deployment.
#
# The deploy script runs one of these services at a time. Caddy on the host
# proxies to both loopback ports, with 127.0.0.1:18080 first and
# 127.0.0.1:18081 as the failover upstream. Both bind only to loopback; no
# application port is reachable from outside the host.
name: open-unleashed

x-game: &game
  image: ${OPEN_UNLEASHED_IMAGE:?OPEN_UNLEASHED_IMAGE must be set by the deploy script}
  volumes:
    - ${NFS_WEB_DATA:-/srv/open-unleashed/web-data}:/usr/share/nginx/html/data:ro
  restart: unless-stopped
  read_only: true
  tmpfs:
    - /tmp:rw,noexec,nosuid,size=16m
    - /var/cache/nginx:rw,noexec,nosuid,size=32m
    - /run:rw,noexec,nosuid,size=8m
  security_opt:
    - no-new-privileges:true
  stop_grace_period: 30s
  logging:
    driver: json-file
    options:
      max-size: "10m"
      max-file: "3"
  healthcheck:
    test: ["CMD", "wget", "-q", "-T", "2", "-O", "/dev/null", "http://127.0.0.1:8080/healthz"]
    interval: 10s
    timeout: 3s
    retries: 3
    start_period: 10s

services:
  blue:
    <<: *game
    ports:
      - "127.0.0.1:${BLUE_PORT:-18080}:8080"

  green:
    <<: *game
    ports:
      - "127.0.0.1:${GREEN_PORT:-18081}:8080"
OPEN_UNLEASHED_COMPOSE_EOF
chmod 0644 "$COMPOSE_FILE"

cat > "$DEPLOY_SCRIPT" <<'OPEN_UNLEASHED_REMOTE_EOF'
#!/usr/bin/env bash
# Remote deployment helper for the OpenUnleashed web build.
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
OPEN_UNLEASHED_REMOTE_EOF
chmod 0755 "$DEPLOY_SCRIPT"

if [[ -f "$ENV_FILE" ]]; then
    echo "Keeping existing $ENV_FILE."
else
cat > "$ENV_FILE" <<'OPEN_UNLEASHED_ENV_EOF'
# Server-side configuration for the OpenUnleashed web deployment.
# Installed as /etc/open-unleashed/deploy.env. Contains no secrets.

# Prepared original game data. Mounted read-only into both containers.
NFS_WEB_DATA=/srv/open-unleashed/web-data

# Loopback ports used by Caddy. The order in the Caddyfile is significant:
# blue first, green second.
BLUE_PORT=18080
GREEN_PORT=18081

# Optional public health check performed after switching upstreams.
# DEPLOY_HEALTH_URL=https://porsche.nfs.place/healthz

# Optional tuning.
# OPEN_UNLEASHED_IMAGE_REPO=ghcr.io/vladislav-yashin/wasm-unleashed
# OPEN_UNLEASHED_HEALTH_TIMEOUT=90
# OPEN_UNLEASHED_SETTLE_SECONDS=10
# OPEN_UNLEASHED_DRAIN_SECONDS=10
# OPEN_UNLEASHED_STOP_GRACE_SECONDS=30
OPEN_UNLEASHED_ENV_EOF
chmod 0644 "$ENV_FILE"
    echo "Installed $ENV_FILE. Review the data path and ports."
fi

USER_HOME="$(getent passwd "$DEPLOY_USER" | cut -d: -f6)"
SSH_DIR="$USER_HOME/.ssh"
KEY_FILE="$SSH_DIR/id_ed25519"
AUTHORIZED_KEYS="$SSH_DIR/authorized_keys"

install -d -m 0700 -o "$DEPLOY_USER" -g "$DEPLOY_USER" "$SSH_DIR"
if [[ ! -f "$KEY_FILE" ]]; then
    ssh-keygen -t ed25519 -N "" -f "$KEY_FILE" -C "open-unleashed-deploy" >/dev/null
    echo "Created CI SSH key: $KEY_FILE"
else
    echo "Keeping existing CI SSH key: $KEY_FILE"
fi

if [[ ! -f "${KEY_FILE}.pub" ]]; then
    ssh-keygen -y -f "$KEY_FILE" > "${KEY_FILE}.pub"
fi

PUBLIC_KEY="$(cat "${KEY_FILE}.pub")"
RESTRICTED_ENTRY="restrict,command=\"$DEPLOY_SCRIPT\" $PUBLIC_KEY"

touch "$AUTHORIZED_KEYS"
# Remove any previous copy of this key, including unrestricted entries.
if grep -qF "$PUBLIC_KEY" "$AUTHORIZED_KEYS"; then
    grep -vF "$PUBLIC_KEY" "$AUTHORIZED_KEYS" > "$AUTHORIZED_KEYS.tmp" || true
    mv "$AUTHORIZED_KEYS.tmp" "$AUTHORIZED_KEYS"
fi
printf '%s\n' "$RESTRICTED_ENTRY" >> "$AUTHORIZED_KEYS"
chown "$DEPLOY_USER:$DEPLOY_USER" "$KEY_FILE" "${KEY_FILE}.pub" "$AUTHORIZED_KEYS"
chmod 0600 "$KEY_FILE" "$AUTHORIZED_KEYS"
chmod 0644 "${KEY_FILE}.pub"

if [[ -z "$SSH_HOST" ]]; then
    SSH_HOST="$(curl -fsS --max-time 5 https://ifconfig.me 2>/dev/null || hostname -I | awk '{print $1}')"
fi

if [[ -f /etc/ssh/ssh_host_ed25519_key.pub ]]; then
    HOST_KEY="$(cat /etc/ssh/ssh_host_ed25519_key.pub)"
    if [[ "$SSH_PORT" == "22" ]]; then
        KNOWN_HOSTS="$SSH_HOST $HOST_KEY"
    else
        KNOWN_HOSTS="[$SSH_HOST]:$SSH_PORT $HOST_KEY"
    fi
else
    KNOWN_HOSTS=""
fi

cat <<EOF

====================================================
OpenUnleashed deploy setup is complete.

GitHub production environment secrets:

DEPLOY_HOST=$SSH_HOST
DEPLOY_USER=$DEPLOY_USER
DEPLOY_SSH_PORT=$SSH_PORT
DEPLOY_SSH_KEY:
$(cat "$KEY_FILE")
EOF

if [[ -n "$KNOWN_HOSTS" ]]; then
    cat <<EOF
DEPLOY_KNOWN_HOSTS:
$KNOWN_HOSTS
EOF
else
    echo "DEPLOY_KNOWN_HOSTS: generate it on a trusted machine with"
    echo "  ssh-keyscan -H -p $SSH_PORT $SSH_HOST"
fi

cat <<EOF
====================================================

Caddy upstreams for porsche.nfs.place:
  127.0.0.1:18080  (blue)
  127.0.0.1:18081  (green)

Original game data must be prepared separately under:
  $DATA_DIR

The CI key is restricted to this command (the deploy user is in the docker
group and needs no sudo):
  $DEPLOY_SCRIPT

After storing the private key in GitHub, remove it from the server:
  rm $KEY_FILE

The deploy user was added to the docker group. Log in again (or start a new
SSH session) before using docker as $DEPLOY_USER.
EOF
