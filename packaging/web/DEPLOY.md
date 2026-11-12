# Browser deployment

This document describes the production deployment for the browser build. It
uses GitHub Actions, GHCR, a restricted SSH key and a blue-green pair of nginx
containers behind Caddy on the host.

## Flow

```
push to main
  test/build WASM and web modules
  build and push ghcr.io/<owner>/<repo>@sha256:...
  wait for the production environment approval
  ssh deploy@host "deploy ghcr.io/<owner>/<repo>@sha256:..."
```

The server script pulls the exact digest, starts the inactive blue-green slot,
waits for its health check, drains the active slot and only then stops it.
Caddy sends traffic to the first healthy upstream, so the switch does not rely
on a rolling `docker compose up` restart.

PRs and pushes to other branches run tests and the WASM build, but cannot reach
the production environment or its secrets.

## Server prerequisites

- Linux with Docker Engine and the `docker compose` plugin.
- `flock` from util-linux.
- Caddy running on the host and terminating HTTPS.
- Prepared original game data under a directory mounted at `/data`.
- `curl` on the host if `DEPLOY_HEALTH_URL` is configured.

The application container never publishes a port on a public interface. The
production compose file binds both slots to loopback:

- `127.0.0.1:18080` - blue
- `127.0.0.1:18081` - green

Ports and the data path are configured in `/etc/open-unleashed/deploy.env`.
Enable `DEPLOY_HEALTH_URL=https://porsche.nfs.place/healthz` after the Caddy
configuration has been applied; this makes the deploy verify the switch through
the real HTTPS endpoint.

## One-time server setup

`packaging/web/deploy/setup-server.sh` is self-contained. Copy that one file to
the server; no repository checkout is needed there:

```sh
scp packaging/web/deploy/setup-server.sh server:/tmp/
ssh server 'sudo OPEN_UNLEASHED_SSH_HOST=your.host.example \
  bash /tmp/setup-server.sh'
```

Set `OPEN_UNLEASHED_SSH_HOST` and `OPEN_UNLEASHED_SSH_PORT` if the server is not
reached by its default IP and port 22; the printed `DEPLOY_KNOWN_HOSTS` must
match the `DEPLOY_HOST` value used by the workflow.

The compose file, remote helper and default `deploy.env` are embedded in the
generated setup script. Their editable sources are
`packaging/web/compose.deploy.yaml`,
`packaging/web/deploy/remote-deploy.sh` and
`packaging/web/deploy/deploy.env.example`. After changing a source, regenerate
the standalone script and commit it:

```sh
python3 packaging/web/deploy/generate-setup.py
```

CI runs `generate-setup.py --check` and fails if the embedded copy is stale.

The script:

- creates the `open-unleashed-deployer` user and adds it to the `docker` group;
- installs `/opt/open-unleashed/compose.yaml`;
- installs `/usr/local/bin/open-unleashed-deploy`;
- installs `/etc/open-unleashed/deploy.env` with the default data path and ports;
- creates `/var/lib/open-unleashed` for the active-slot state;
- creates an ed25519 key for GitHub Actions;
- adds the public key to `authorized_keys` with a forced command and `restrict`.

The authorized key can only run:

```text
/usr/local/bin/open-unleashed-deploy
```

The helper accepts one argument: a digest from the configured GHCR repository.
It rejects other commands and never evaluates input as shell code. It also
requires the prepared data directory to contain `manifest.json`.

The script prints the values for the GitHub production environment. Store the
private key in `DEPLOY_SSH_KEY`, then remove it from the server. Re-run the
script after changing `compose.deploy.yaml` or `remote-deploy.sh`; it preserves
the existing `deploy.env` and SSH key.

Prepare the original game data locally. The `web-data` target stages only the
files the browser build consumes, filters unneeded installation files, and
validates `nfs5.exe`/`Porsche.exe`/`PORSCHE.ICD` before extracting the movie
tables. Pass `MOVIE_EXE=...` only to override the auto-detected executable:

```sh
make web-data GAME_DIR=/path/to/original/game

sudo rsync -a --delete web-data/ user@your.host:/srv/open-unleashed/web-data/
sudo chmod -R a+rX /srv/open-unleashed/web-data
```

The packer reuses existing content-addressed objects and does not delete stale
ones. For a clean data directory, build into a fresh path and replace it
afterwards:

```sh
make web-data GAME_DIR=/path/to/original/game WEB_DATA=web-data.new
rm -rf web-data
mv web-data.new web-data
```

Original data is never part of the image or of a GitHub artifact. It is mounted
read-only into both containers. Keep it outside the repository.

## GitHub environment

Create an environment named `production` in the repository settings and add a
required reviewer. Add these environment secrets:

| Secret               | Required | Description                                      |
| -------------------- | -------- | ------------------------------------------------ |
| `DEPLOY_HOST`        | yes      | Server hostname or IP                            |
| `DEPLOY_USER`        | yes      | `open-unleashed-deployer`                        |
| `DEPLOY_SSH_PORT`    | no       | SSH port, defaults to `22`                       |
| `DEPLOY_SSH_KEY`     | yes      | Private ed25519 key printed by `setup-server.sh` |
| `DEPLOY_KNOWN_HOSTS` | yes      | Pinned server host key in `known_hosts` format   |

Generate `DEPLOY_KNOWN_HOSTS` on a trusted machine, not inside the workflow:

```sh
ssh-keyscan -H -p 22 your.host.example
```

The workflow never runs `ssh-keyscan` and never disables host key checking.
The deploy job does not run for pull requests. Protect `main` and limit who can
approve the `production` environment: anyone who can change the workflow and
approve a deployment can use the deploy key, even though the key itself is
restricted to one remote command.

The image is public, so the server needs no registry credentials. After the
first image push, open the package settings of the GHCR package and set its
visibility to public before approving the first deployment. The image contains
only the open-source web build; it contains no original game data.

## Caddy

The real Caddyfile is kept in the separate Caddy configuration repository. The
required contract is shown in
[`deploy/Caddyfile.example`](deploy/Caddyfile.example):

- blue is the first upstream, green is the second;
- `lb_policy first` is required, not `random` or `round_robin`;
- active health checks use `/healthz`;
- `lb_try_duration` keeps requests alive while Caddy selects the other slot.

The deploy helper relies on these timings. If the health-check interval is
changed, adjust `OPEN_UNLEASHED_SETTLE_SECONDS` and
`OPEN_UNLEASHED_DRAIN_SECONDS` in `/etc/open-unleashed/deploy.env`.

## Deployment sequence

1. The inactive slot is removed if a previous failed attempt left it behind.
2. The new digest is pulled.
3. The inactive slot starts with the new image.
4. Docker health and the internal `index.html` request must succeed.
5. Caddy is given time to observe the new upstream.
6. The old slot is put into draining mode (`/tmp/draining` makes `/healthz`
   return 503), so Caddy removes it before it is stopped.
7. An optional public health check runs through Caddy.
8. The active-slot state is written and the old container is stopped with
   `SIGQUIT`.

If startup or health checks fail, the new container is removed and the old slot
keeps serving. If the public probe fails after a drain, the helper restores the
old upstream before removing the new one.

The previous image digest is stored in `/var/lib/open-unleashed/image.previous`.

## Operation

Useful state files and commands:

```sh
cat /var/lib/open-unleashed/active
cat /var/lib/open-unleashed/image
cat /var/lib/open-unleashed/image.previous

docker ps --filter name=open-unleashed
docker logs --tail=100 open-unleashed-blue-1
docker logs --tail=100 open-unleashed-green-1

# Compose commands need the image reference used by the running container.
export OPEN_UNLEASHED_IMAGE="$(cat /var/lib/open-unleashed/image)"
cd /opt/open-unleashed
docker compose --env-file /etc/open-unleashed/deploy.env ps
```

Manual rollback to an older digest:

```sh
sudo -u open-unleashed-deployer /usr/local/bin/open-unleashed-deploy \
  ghcr.io/vladislav-yashin/wasm-unleashed@sha256:...
```

The helper alternates slots, so a rollback is the same blue-green operation as a
normal deployment.

If a deployment was interrupted and both containers are running, the helper
refuses to guess. Inspect both slots, remove the stale one, and confirm that
`/var/lib/open-unleashed/active` names the container that should serve traffic.

## Data and saves

- `/srv/open-unleashed/web-data` is read-only and contains prepared assets.
- Profiles, records and settings stay in browser IndexedDB; the server has no
  writable game-data volume.
- Music archives remain segmented, as described in the local web build README.

## Validation

Before relying on the deployment, check locally:

```sh
bash -n packaging/web/deploy/remote-deploy.sh
bash -n packaging/web/deploy/setup-server.sh
OPEN_UNLEASHED_IMAGE='ghcr.io/example/open-unleashed@sha256:0000000000000000000000000000000000000000000000000000000000000000' \
  docker compose -f packaging/web/compose.deploy.yaml config \
  --env-file packaging/web/deploy/deploy.env.example
```

The CI image job also starts the pushed image with the production read-only and
tmpfs settings, checks `/healthz`, checks `index.html`, and verifies that the
draining marker changes `/healthz` to 503.
