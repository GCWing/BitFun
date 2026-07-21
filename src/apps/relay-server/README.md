# BitFun Relay Server

WebSocket / HTTP relay for BitFun **Remote Connect** and **account login**.

Open-source BitFun does **not** ship a public hosted login service. If you want
Desktop / CLI **account login**, cross-device session & settings sync, or
**Peer Device Mode** (control another online device on the same account), you
must:

1. Deploy this relay yourself
2. Enable the account database (`RELAY_DB_PATH`)
3. Create user accounts out-of-band with `relay-admin` (no public sign-up)
4. Point BitFun Desktop or CLI at your relay URL and log in

The relay stays **zero-knowledge**: clients encrypt with a master key derived
locally; the server stores Argon2id password hashes and AES-GCM-wrapped keys,
never plaintext passwords or decryptable sync payloads.

## Supported deploy hosts

One-click Docker deploy (`bash deploy.sh`) targets:

| OS | CPU |
|----|-----|
| Linux | **amd64** (`x86_64`) |
| Linux | **arm64** (`aarch64`) |

Requirements: Docker Engine + Compose V2 (`docker compose`) **or** legacy
`docker-compose`, plus permission to talk to the Docker daemon.

Build natively on the server (do **not** set `DOCKER_DEFAULT_PLATFORM` to a
foreign arch unless you intentionally cross-build with qemu). On small
memory VPS (common on arm64), use:

```bash
RELAY_CARGO_BUILD_JOBS=1 bash deploy.sh
```

## Two operating modes

| Mode | When | What you get |
|------|------|----------------|
| **Pure relay** | `RELAY_DB_PATH` unset | Room pairing + mobile HTTP ↔ Desktop WebSocket bridge only. **No** account login, sync, or Peer Device Mode. |
| **Account-enabled** | `RELAY_DB_PATH` set to a persistent SQLite path | Everything above **plus** login, device presence, device RPC (Peer HostInvoke), encrypted session/settings sync. |

Docker Compose in this directory **already enables account mode**
(`RELAY_DB_PATH=/app/data/bitfun_relay.db`). Manual / cargo runs must set the
variable yourself or accounts stay disabled.

## Features

- Desktop and CLI connect via WebSocket; mobile uses HTTP
- End-to-end encrypted passthrough (the server does not decrypt payloads)
- Correlation-based HTTP-to-WebSocket request-response matching
- Per-room mobile-web static file upload and serving
- Heartbeat-based connection management with configurable room TTL
- Optional zero-knowledge account storage + device routing + sync
- Docker deployment support with optional Caddy reverse proxy

## Open-source: enable account login (recommended path)

Use this checklist on a machine you control (VPS, LAN server, or localhost).

### Desktop one-click deploy (preferred for end users)

BitFun Desktop can SSH to your host and run the same Docker path without a
manual clone. Entry points: Account Login → “一键部署到自己的服务器”, or
Remote Connect → Network Relay → Self-Hosted → the same action.

- Orchestration: `src/crates/services/services-integrations/src/remote_ssh/relay_deploy.rs`
- Wizard + invariants: `src/web-ui/src/features/relay-deploy/README.md`

Remote checkout path is always `~/.bitfun/relay-src` (never `$HOME/BitFun`).
Closing the wizard cancels the remote task. Account passwords are provisioned
locally and imported via `relay-admin import-user`.

### 1. Deploy the relay (manual / server shell)

```bash
git clone https://github.com/GCWing/BitFun
cd BitFun/src/apps/relay-server
bash deploy.sh
```

`deploy.sh` must run **on the target server** (it does not SSH elsewhere).
Requires Docker and Docker Compose on **linux/amd64** or **linux/arm64**.

After a successful start, the script runs `relay-admin list-users`. If the
database has **no accounts**, it prints the exact `add-user` command to run
next (account login will not work until you create at least one user).

Verify:

```bash
curl -fsS http://127.0.0.1:9700/health
docker compose ps
```

### 2. Confirm account database is on

Compose sets:

```yaml
RELAY_DB_PATH=/app/data/bitfun_relay.db
```

Data lives in the `relay-db` Docker volume. If you run the binary without
Compose, export a persistent path first:

```bash
export RELAY_DB_PATH=/var/lib/bitfun/bitfun_relay.db
mkdir -p "$(dirname "$RELAY_DB_PATH")"
RELAY_PORT=9700 ./target/release/bitfun-relay-server
```

If the process logs `RELAY_DB_PATH not set — account features disabled`, login
will fail with “account features disabled” until you fix the env and restart.

### 3. Create accounts (`relay-admin`)

There is **no** public registration API. Operators create users with
`relay-admin` (bundled in the Docker image). `--db` must be the **same path**
as `RELAY_DB_PATH`.

```bash
# Interactive password prompt (recommended)
docker exec -it bitfun-relay \
  /app/relay-admin --db /app/data/bitfun_relay.db add-user --username alice

# Non-interactive (scripts / CI)
docker exec bitfun-relay \
  /app/relay-admin --db /app/data/bitfun_relay.db add-user \
  --username alice --password 'choose-a-strong-password'

# List accounts
docker exec bitfun-relay \
  /app/relay-admin --db /app/data/bitfun_relay.db list-users
```

Other commands:

```bash
# Reset password (also rotates the master key — old synced blobs become unreadable)
docker exec -it bitfun-relay \
  /app/relay-admin --db /app/data/bitfun_relay.db reset-password --username alice

# Rename (credentials / user_id unchanged)
docker exec bitfun-relay \
  /app/relay-admin --db /app/data/bitfun_relay.db rename-user \
  --username alice --new-username alice2

# Delete account and all of its relay-side data
docker exec bitfun-relay \
  /app/relay-admin --db /app/data/bitfun_relay.db delete-user --username alice
```

Without Docker, build and run the same tool from this crate:

```bash
cargo build --release -p bitfun-relay-server
./target/release/relay-admin --db "$RELAY_DB_PATH" add-user --username alice
```

### 4. Point BitFun clients at your relay

Relay URL examples:

- Direct: `http://<YOUR_SERVER_IP>:9700`
- Localhost: `http://127.0.0.1:9700`
- Behind a reverse proxy: `https://relay.example.com/relay`

The client appends paths (`/ws`, `/api/*`, `/r/*`) to the URL you enter. Use
the `/relay` suffix to match the official server format
(`https://remote.openbitfun.com/relay`). See **Reverse Proxy** for nginx config.

**Desktop**

1. Open account / login UI (or Remote Connect self-hosted settings, depending
   on your build).
2. Set **Auth Server / Relay URL** to the URL above.
3. Sign in with the username and password you created with `relay-admin`.

**CLI**

1. Run `bitfun`, open `/login`.
2. Fill **Auth Server**, **Username**, **Password**, then Login.
3. After login, the CLI can act as a **Peer Host** for same-account Desktops.

Clients remember a non-secret hint (`~/.bitfun/account_hint.json`: username +
relay URL) and an encrypted session file for restart without retyping the
password.

### 5. What works after login

- Encrypted **settings / session sync** across devices on the same account
- **Device list** and online presence for that account
- **Peer Device Mode**: one Desktop controls another online Desktop **or** CLI
  host over device RPC (`HostInvoke` / `DeviceEvent`)
- Same machine Desktop + CLI share one `device_id`; the **last successful**
  `AuthConnect` wins as the live Peer Host for that id

## Upgrade notes

The supported Docker build context is now the repository root because the app
uses the shared relay service:

```bash
docker build -f src/apps/relay-server/Dockerfile .
docker compose -f src/apps/relay-server/docker-compose.yml build
```

Copying only `src/apps/relay-server` is no longer sufficient; deployments must
also include `src/crates/services/relay-service`. The repository keeps one
Docker build layout rather than duplicating the shared service.

The Rust crate path `bitfun_relay_server` remains as a thin compatibility
facade, including its existing module paths and four-argument router builder.
New library consumers should depend on `bitfun-relay-service`.

## Quick Start (service ops)

### Recommended: Run on the target server

```bash
git clone https://github.com/GCWing/BitFun
cd BitFun/src/apps/relay-server
bash deploy.sh
```

### Service Operations

Run these on the target server inside this directory:

```bash
bash start.sh
bash stop.sh
bash restart.sh
docker compose ps
docker compose logs -f relay-server
```

Notes:

- `start.sh` is idempotent and exits if the service is already running.
- `stop.sh` exits cleanly when the service is already stopped.
- `restart.sh` restarts the service when running, or starts it when stopped.
- The container uses `restart: unless-stopped`.

### Network Binding

By default the relay listens on `0.0.0.0:9700` and Compose publishes that port
on the host.

Restrict to localhost:

```bash
export RELAY_HOST_BIND_IP=127.0.0.1
bash deploy.sh
```

### Manual Run (without Docker)

```bash
# From repository root
cargo build --release -p bitfun-relay-server

# Account-enabled (persistent DB path required for login)
export RELAY_DB_PATH="$HOME/.bitfun-relay/bitfun_relay.db"
mkdir -p "$(dirname "$RELAY_DB_PATH")"
RELAY_PORT=9700 ./target/release/bitfun-relay-server
```

## Deployment Checklist

1. Open ports: `9700` (direct), and `80/443` if using a reverse proxy.
2. Hit `http://<server-ip>:9700/health` (or `https://relay.example.com/relay/health` behind a proxy).
3. Confirm `RELAY_DB_PATH` if you need accounts (Compose does this for you).
4. Create at least one user with `relay-admin`.
5. Fill the same relay URL into Desktop / CLI and log in.
6. If you terminate TLS on a reverse proxy, raise body size and read timeouts
   (see sync + device RPC notes below).
7. Use the `/relay` suffix in the relay URL (e.g. `https://relay.example.com/relay`)
   to match the official server format. See **Reverse Proxy** for nginx config.

## Reverse Proxy

When deploying behind a reverse proxy (Caddy, nginx, etc.), configure:

- **Body size limit**: at least 100 MB (sync POSTs carry large encrypted bundles)
- **Read/response timeout**: at least 130s (device RPC waits up to 120s)
- **WebSocket upgrade**: the /ws endpoint requires Connection upgrade headers
- **Path prefix**: serve the relay at `/relay/*` (strip prefix before proxying
  to port 9700); serve static homepage files at `/` via exact-match locations

### Nginx example (/relay prefix + homepage at /)

```nginx
server {
    listen 80;
    server_name relay.example.com;

    # Homepage static files (exact match)
    location = / {
        root /path/to/relay-server/static/homepage;
        try_files /index.html =404;
    }
    location = /i18n.json {
        root /path/to/relay-server/static/homepage;
    }
    location = /i18n.shared.json {
        root /path/to/relay-server/static/homepage;
    }

    # With /relay prefix: strip prefix, proxy to relay server
    # For clients configured with https://relay.example.com/relay
    location = /relay {
        return 301 /relay/;
    }
    location /relay/ {
        proxy_pass http://127.0.0.1:9700/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_buffering off;
        proxy_read_timeout 130s;
        proxy_send_timeout 130s;
        client_max_body_size 100m;
    }

}
```

See `Caddyfile` for the Caddy equivalent.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RELAY_PORT` | `9700` | Server listen port |
| `RELAY_STATIC_DIR` | _(none)_ | Path to mobile web static files fallback SPA. When unset, no fallback static files are served. Docker Compose sets this to `/app/static`. |
| `RELAY_ROOM_WEB_DIR` | `/tmp/bitfun-room-web` | Directory for per-room uploaded mobile-web files. Docker Compose uses a named volume mounted at `/app/room-web`. |
| `RELAY_ASSET_STORE_MAX_BYTES` | `1073741824` | Global content-addressed asset capacity (1 GiB by default). New uploads return HTTP 507 after the limit is reached; existing content remains readable. |
| `RELAY_ROOM_TTL` | `300` | Idle room TTL in seconds (0 = no expiry). Active heartbeats and commands refresh activity. |
| `RELAY_DB_PATH` | _(none)_ | SQLite path for account storage. **Unset = pure relay (no login).** Set a persistent path (Compose: `/app/data/bitfun_relay.db`) to enable login, device routing, and sync. Accounts are provisioned only via `relay-admin`. |
| `RELAY_CORS_ALLOW_ORIGINS` | _(none)_ | Comma-separated browser origin allowlist, for example `https://remote.example.com`. Empty means same-origin only. `*` is rejected when account APIs are enabled. |

When `RELAY_DB_PATH` is set, database open or migration failure is fatal: the
process exits instead of silently starting without account protection. The
`/health` response reports account capability, room/device connection counts,
pending bridge requests, and asset-store used/capacity bytes for operational
checks and capacity alerts.

## API Endpoints

### Health & Info

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check (status, version, uptime, room and connection counts) |
| `/api/info` | GET | Server info (name, version, protocol version) |

### Account (requires `RELAY_DB_PATH`)

Zero-knowledge authentication. Clients derive an Argon2id KEK locally and send
only password hashes. Brute-force protection: per-account lockout + per-IP rate
limit. **No public registration endpoint.**

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/auth/login/challenge` | POST | Fetch KDF params + wrapped master key for local derivation |
| `/api/auth/login` | POST | Verify password hash and issue a token; returns `{ token, user_id }` |
| `/api/auth/logout` | POST | Revoke the caller's token |
| `/api/auth/delegate` | POST | Issue a delegated token for a paired client (authenticated caller) |

### Devices (requires `RELAY_DB_PATH` + Bearer token)

Used by Desktop / CLI / mobile-web for presence and Peer Device Mode RPC.

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/devices` | GET | List devices for the account (online + offline) |
| `/api/devices/:target_device_id/rpc` | POST | Route an opaque encrypted RPC to an **online** device (waits up to **120s**) |
| `/api/devices/:target_device_id` | DELETE | Remove a device registration (and drop any live WS session) |

#### Device RPC timeouts (Peer HostInvoke)

`POST /api/devices/:target_device_id/rpc` waits up to **120 seconds** for the
target device (`RPC_TIMEOUT` in
`../../crates/services/relay-service/src/routes/devices.rs`). Peer Device Mode
uses this for product `invoke` calls.

Reverse proxies in front of the relay must use a read / response timeout
**≥ 120s** (recommend 130s), or clients see **HTTP 504** before Axum finishes.
See `Caddyfile` for `transport http` timeout settings.

### Room Operations (Mobile HTTP → Desktop WS bridge)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/rooms/:room_id/pair` | POST | Mobile initiates pairing; relay forwards to desktop via WebSocket and waits for a response |
| `/api/rooms/:room_id/command` | POST | Mobile sends an encrypted command; relay forwards it to desktop and returns the response |

### Per-Room Mobile-Web File Management

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/rooms/:room_id/upload-web` | POST | Full upload of base64-encoded files keyed by path (10 MB body limit) |
| `/api/rooms/:room_id/check-web-files` | POST | Incremental check for already uploaded files by hash |
| `/api/rooms/:room_id/upload-web-files` | POST | Incremental upload of only missing files (10 MB body limit) |
| `/r/:room_id/*path` | GET | Serve uploaded mobile-web static files for a room |

### WebSocket

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/ws` | WebSocket | Desktop **and CLI** account / room clients |

### Cross-Device Sync (requires `RELAY_DB_PATH` + Bearer token)

Encrypted session and settings blobs. All payloads are AES-256-GCM encrypted
client-side with the account master key; the relay cannot read them.

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/sync/sessions` | POST | Upload/replace an encrypted session blob (**64 MiB** Axum body limit) |
| `/api/sync/sessions` | GET | List encrypted session blobs (`?since=<version>`) |
| `/api/sync/sessions/:session_id` | GET | Fetch one encrypted session blob by id |
| `/api/sync/sessions/:session_id` | DELETE | Soft-delete a session blob (tombstone) |
| `/api/sync/settings` | POST | Upload/replace the encrypted settings blob (**64 MiB** Axum body limit) |
| `/api/sync/settings` | GET | Fetch the encrypted settings blob |

#### Request body size limits (Axum vs reverse proxy)

Session sync posts a **full** encrypted session bundle. Large conversations can
exceed Axum’s default ~2 MiB limit and fail with **HTTP 413**.

This server raises the limit on sync POSTs to **64 MiB** (`SYNC_BODY_LIMIT` in
`../../crates/services/relay-service/src/routes/sync.rs`). Proxies must raise
their body limit too, or they reject uploads before Axum sees them:

```nginx
# nginx — must be >= Axum SYNC_BODY_LIMIT (64M)
client_max_body_size 100M;
```

```caddy
# Caddy: request_body { max_size 100MB }
```

When diagnosing 413s, check **both** the proxy and Axum. Direct host-port access
only hits the Axum limit.

## WebSocket Protocol

Desktop and CLI use WebSocket for rooms and/or account device routing. Mobile
clients use the HTTP endpoints above.

### Client → Server (Inbound)

```json
// Create a room (Remote Connect room bridge)
{ "type": "create_room", "room_id": "optional-id", "device_id": "...", "device_type": "desktop", "public_key": "base64..." }

// Respond to a bridged HTTP request (pair or command)
{ "type": "relay_response", "correlation_id": "...", "encrypted_data": "base64...", "nonce": "base64..." }

// Heartbeat
{ "type": "heartbeat" }

// Account-authenticated device routing (requires RELAY_DB_PATH)
{ "type": "auth_connect", "token": "...", "device_name": "..." }
{ "type": "device_message", "target_device_id": "...", "correlation_id": "...", "encrypted_data": "base64...", "nonce": "base64..." }
```

A second `auth_connect` with the same `(user_id, device_id)` **replaces** the
previous live connection (last connect wins).

### Server → Client (Outbound)

```json
{ "type": "room_created", "room_id": "..." }
{ "type": "pair_request", "correlation_id": "...", "public_key": "base64...", "device_id": "...", "device_name": "..." }
{ "type": "command", "correlation_id": "...", "encrypted_data": "base64...", "nonce": "base64..." }
{ "type": "heartbeat_ack" }
{ "type": "auth_ok", "user_id": "...", "device_id": "..." }
{ "type": "auth_error", "message": "..." }
{ "type": "incoming_device_message", "source_device_id": "...", "correlation_id": "...", "encrypted_data": "base64...", "nonce": "base64..." }
{ "type": "device_presence", "devices": [{ "device_id": "...", "device_name": "..." }] }
{ "type": "error", "message": "..." }
```

## Architecture

```
Mobile ──HTTP──► Relay ◄──WebSocket── Desktop / CLI
                   │
              opaque E2E payloads
              (optional SQLite for
               accounts / sync / devices)
```

- **Room bridge**: Desktop creates a room; mobile posts `/pair` and `/command`;
  the relay correlates HTTP ↔ WebSocket without reading ciphertext.
- **Account plane** (when `RELAY_DB_PATH` is set): clients log in over HTTP,
  then `auth_connect` on WebSocket; device RPC and sync store opaque blobs.
- Per-room mobile-web files can be served at `/r/:room_id/`.

## Directory structure

```
relay-server/
├── src/
│   ├── main.rs             # Relay server binary entry point
│   ├── config.rs           # Environment-based configuration
│   └── bin/
│       └── relay_admin.rs  # relay-admin CLI binary
├── static/                 # Mobile-web static files
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml      # Sets RELAY_DB_PATH for account mode
├── Caddyfile               # Optional reverse proxy (body + RPC timeouts)
├── deploy.sh
├── start.sh / stop.sh / restart.sh
├── common.sh               # Shared helpers for the scripts above
└── README.md
```

Reusable relay state, storage, asset stores, and HTTP/WebSocket routes live in
`src/crates/services/relay-service`. This directory owns only the standalone
process configuration, static-file fallback, and operator CLI.

## About `src/apps/server` vs `src/apps/relay-server`

- Self-hosted Remote Connect **and** open-source account login use **this**
  `relay-server` directory.
- `src/apps/server` is a different application and is not the relay used by
  Desktop / CLI / mobile Remote Connect.
