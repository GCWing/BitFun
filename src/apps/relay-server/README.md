# BitFun Relay Server

WebSocket relay server for BitFun Remote Connect. It bridges desktop (WebSocket) and mobile (HTTP) clients while forwarding end-to-end encrypted payloads.

## Features

- Desktop connects via WebSocket, mobile via HTTP
- End-to-end encrypted passthrough (the server does not decrypt payloads)
- Correlation-based HTTP-to-WebSocket request-response matching
- Per-room mobile-web static file upload and serving
- Heartbeat-based connection management with configurable room TTL
- Optional zero-knowledge account storage (E2E encrypted — the server never sees passwords or master keys)
- Docker deployment support with optional Caddy reverse proxy

## Quick Start

### Recommended: Run on the target server

```bash
# Clone on the target server
git clone https://github.com/GCWing/BitFun
cd BitFun/src/apps/relay-server

# Deploy to the current machine
bash deploy.sh
```

`deploy.sh` must run on the target server itself. It deploys to the current machine only and does not SSH to another host.

### Service Operations

Run these commands on the target server inside this directory:

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

### What URL should I fill in BitFun Desktop?

In **Remote Connect → Self-Hosted → Server URL**, use one of:

- `http://<YOUR_SERVER_IP>:9700`

`/relay` is only needed when your reverse proxy is configured with that path prefix.

### Network Binding

By default, the relay process listens on `0.0.0.0:9700` (all interfaces) and Docker Compose publishes the container port on the host's `0.0.0.0:9700`.

If you need to restrict the service to localhost only, set the environment variable before running `start.sh`/`restart.sh`/`deploy.sh`:

```bash
export RELAY_HOST_BIND_IP=127.0.0.1
bash deploy.sh
```

### Manual Run

```bash
# From project root
cargo build --release -p bitfun-relay-server

# Run
RELAY_PORT=9700 ./target/release/bitfun-relay-server
```

## Deployment Checklist

1. Open required ports:
   - `9700` for direct relay access
   - `80/443` when using Caddy or another reverse proxy
2. Verify the health endpoint:
   - `http://<server-ip>:9700/health`
3. Decide the final URL strategy:
   - direct port or reverse proxy domain
4. Fill the same URL into BitFun Desktop custom server settings

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RELAY_PORT` | `9700` | Server listen port |
| `RELAY_STATIC_DIR` | _(none)_ | Path to mobile web static files fallback SPA. When unset, no fallback static files are served. Docker Compose sets this to `/app/static`. |
| `RELAY_ROOM_WEB_DIR` | `/tmp/bitfun-room-web` | Directory for per-room uploaded mobile-web files. Docker Compose uses a named volume mounted at `/app/room-web`. |
| `RELAY_ROOM_TTL` | `3600` | Room TTL in seconds (0 = no expiry) |
| `RELAY_DB_PATH` | _(none)_ | SQLite database path for account storage. When unset, the relay runs in pure-relay mode with no account features. Set to a persistent path (e.g. `/app/data/bitfun_relay.db`) to enable account login, device routing, and cross-device session/settings sync. The server stays zero-knowledge: it only stores Argon2id password hashes and AES-GCM-wrapped master keys — never plaintext passwords or master keys. Accounts are provisioned via the `relay-admin` CLI (see below). |

## API Endpoints

### Health & Info

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check (returns status, version, uptime, room and connection counts) |
| `/api/info` | GET | Server info (name, version, protocol version) |

### Account (optional — requires `RELAY_DB_PATH`)

Zero-knowledge authentication. Clients derive an Argon2id KEK locally and send only password hashes; the server never sees plaintext passwords or master keys. Brute-force protection: per-account exponential-backoff lockout + per-IP sliding-window rate limit. Accounts are **provisioned out-of-band** (no public registration endpoint) so the relay never handles a password.

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/auth/login/challenge` | POST | Fetch KDF params + wrapped master key for local derivation |
| `/api/auth/login` | POST | Verify password hash and issue a token; returns `{ token, user_id }` |

#### Account Provisioning (`relay-admin` CLI)

Accounts are created out-of-band via the `relay-admin` binary (shipped inside the Docker image). No public registration endpoint is exposed.

```bash
# All commands require --db pointing to the same SQLite file as RELAY_DB_PATH

# Add an account (interactive password prompt, recommended)
docker exec -it <container> /app/relay-admin --db /app/data/bitfun_relay.db add-user --username alice

# Add with password on the command line (for scripts)
docker exec <container> /app/relay-admin --db /app/data/bitfun_relay.db add-user --username alice --password "Secret123!"

# List all accounts
docker exec <container> /app/relay-admin --db /app/data/bitfun_relay.db list-users

# Reset password (generates new salts + new master key;
# previously synced encrypted data becomes unreadable)
docker exec -it <container> /app/relay-admin --db /app/data/bitfun_relay.db reset-password --username alice

# Delete an account and all its data
docker exec <container> /app/relay-admin --db /app/data/bitfun_relay.db delete-user --username alice
```

The tool performs the same client-side Argon2id key derivation and AES-256-GCM master-key wrapping as the login client, then writes only non-secret artifacts (salts, hashes, wrapped key) to the database. Passwords are never stored. When `--password` is omitted the tool prompts with hidden input and asks for confirmation.

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
| `/ws` | WebSocket | Desktop client connection endpoint |

### Cross-Device Sync (optional — requires `RELAY_DB_PATH` + Bearer token)

Encrypted session and settings blobs, stored opaquely on the relay. All payloads are AES-256-GCM encrypted client-side with the account master key; the relay cannot read them.

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/sync/sessions` | POST | Upload/replace an encrypted session blob (**64 MiB** Axum body limit) |
| `/api/sync/sessions` | GET | List encrypted session blobs (`?since=<version>`) |
| `/api/sync/sessions/:session_id` | GET | Fetch one encrypted session blob by id |
| `/api/sync/sessions/:session_id` | DELETE | Soft-delete a session blob (tombstone) |
| `/api/sync/settings` | POST | Upload/replace the encrypted settings blob (**64 MiB** Axum body limit) |
| `/api/sync/settings` | GET | Fetch the encrypted settings blob |

#### Request body size limits (Axum vs reverse proxy)

Session sync posts a **full** encrypted session bundle (metadata + all dialog turns). Long conversations with large tool outputs routinely exceed Axum’s default ~2 MiB request body limit and fail with **HTTP 413 Payload Too Large**.

This server raises the limit on `POST /api/sync/sessions` and `POST /api/sync/settings` to **64 MiB** (`SYNC_BODY_LIMIT` in `src/routes/sync.rs`).

If you put nginx / Caddy / another reverse proxy in front of the relay (common for TLS on port 443 → container `9700`), you must also raise the proxy body limit, or the proxy will reject large uploads **before** Axum sees them:

```nginx
# nginx example — must be >= Axum SYNC_BODY_LIMIT (64M)
client_max_body_size 100M;
```

```caddy
# Caddy: request_body { max_size 100MB }
```

When diagnosing 413s, check **both** gates: reverse-proxy `client_max_body_size` (or equivalent) **and** Axum `DefaultBodyLimit`. The effective limit is the stricter of the two. Direct host-port access (e.g. `host:9701` without a proxy) only hits the Axum limit.

#### Device RPC timeouts (Peer HostInvoke)

`POST /api/devices/:target_device_id/rpc` waits up to **120 seconds** for the target device to answer over WebSocket (`RPC_TIMEOUT` in `src/routes/devices.rs`). Peer Device Mode uses this path for every product `invoke`.

If a reverse proxy sits in front of the relay, its read / response timeouts must be **≥ 120s** (recommend 130s), or the proxy returns **HTTP 504 Gateway Timeout** before Axum can. See `Caddyfile` for the `transport http` timeout settings.

## WebSocket Protocol (Desktop Only)

Only desktop clients connect via WebSocket. Mobile clients use the HTTP endpoints above.

### Desktop → Server (Inbound)

```json
// Create a room
{ "type": "create_room", "room_id": "optional-id", "device_id": "...", "device_type": "desktop", "public_key": "base64..." }

// Respond to a bridged HTTP request (pair or command)
{ "type": "relay_response", "correlation_id": "...", "encrypted_data": "base64...", "nonce": "base64..." }

// Heartbeat
{ "type": "heartbeat" }

// Account-authenticated device routing (parallel to room pairing)
{ "type": "auth_connect", "token": "...", "device_name": "..." }
{ "type": "device_message", "target_device_id": "...", "correlation_id": "...", "encrypted_data": "base64...", "nonce": "base64..." }
```

### Server → Desktop (Outbound)

```json
// Room created confirmation
{ "type": "room_created", "room_id": "..." }

// Pair request forwarded from mobile HTTP
{ "type": "pair_request", "correlation_id": "...", "public_key": "base64...", "device_id": "...", "device_name": "..." }

// Encrypted command forwarded from mobile HTTP
{ "type": "command", "correlation_id": "...", "encrypted_data": "base64...", "nonce": "base64..." }

// Heartbeat acknowledgment
{ "type": "heartbeat_ack" }

// Device routing responses
{ "type": "auth_ok", "user_id": "...", "device_id": "..." }
{ "type": "auth_error", "message": "..." }
{ "type": "incoming_device_message", "source_device_id": "...", "correlation_id": "...", "encrypted_data": "base64...", "nonce": "base64..." }
{ "type": "device_presence", "devices": [{ "device_id": "...", "device_name": "..." }] }

// Error
{ "type": "error", "message": "..." }
```

## Architecture

```
Mobile Phone ──HTTP POST──► Relay Server ◄──WebSocket── Desktop Client
                               │
                          E2E Encrypted
                          (server cannot
                           read messages)
```

The relay server bridges HTTP and WebSocket:

- **Desktop** connects via WebSocket, creates a room, and stays connected.
- **Mobile** sends HTTP POST requests such as `/pair` and `/command`.
- The relay forwards requests to the desktop over WebSocket with correlation IDs, waits for the response, and returns it over HTTP.
- The relay only manages rooms and forwards opaque encrypted payloads.
- Per-room mobile-web static files can be uploaded and served at `/r/:room_id/`.

## Directory structure

```
relay-server/
├── src/
│   ├── main.rs             # Relay server binary entry point
│   ├── lib.rs              # Shared library (router, asset stores)
│   ├── config.rs           # Environment-based configuration
│   ├── db.rs               # SQLite account/device/sync storage
│   ├── admin.rs            # Account provisioning crypto (used by relay-admin)
│   ├── bin/
│   │   └── relay_admin.rs  # relay-admin CLI binary
│   ├── relay/              # Room manager + device routing manager
│   └── routes/             # HTTP/WS route handlers (auth, sync, api, websocket)
├── static/                 # Mobile-web static files
├── Cargo.toml              # Crate manifest
├── Dockerfile              # Docker build
├── docker-compose.yml      # Docker Compose config
├── Caddyfile               # Optional reverse proxy config
├── deploy.sh               # Deploy on the target server itself
├── start.sh                # Start service if not already running
├── stop.sh                 # Stop running service
├── restart.sh              # Restart service, or start if stopped
└── README.md
```

## About `src/apps/server` vs `src/apps/relay-server`

- Remote Connect self-hosted deployment uses the relay server in this directory.
- `src/apps/server` is a different application and is not the relay service used by mobile and desktop Remote Connect.
