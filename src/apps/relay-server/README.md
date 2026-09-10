# OpenBitFun Relay Server

The official Relay connects devices signed in to the same GitHub identity.
GitHub identity is shared with the marketplaces. Users sign in
from OpenBitFun; they do not create a Relay account or deploy a server.

The official endpoint is `https://remote.openbitfun.com/v/1.0.0`. This release is deployed with
its own process, database, assets, and reverse-proxy location. An existing
`/relay` deployment remains on its existing binary and data directory.

The Relay forwards opaque encrypted messages. Each device generates an X25519
private key locally and registers its public key after GitHub identity
verification. Peers obtain public keys through the authenticated same-account
directory and derive an AES-GCM key with X25519 and HKDF-SHA256. The Relay does
not receive device private keys or upload copies of settings and sessions.
Sessions and files are read from the owning online device on demand.

Selecting **Same network** starts the same Relay implementation inside the
Desktop host at `http://<LAN-IP>:9700`. Its SQLite database is local to that
host (`<product-home>/relay-v1.0.0/local-server/relay.db`), separate from the
official server database. Login, device registration, device discovery,
public-key lookup, RPC and presence all use the selected Relay endpoint.
The two modes differ only in endpoint and host startup; no device traffic is
forwarded from the local Relay to the official Relay.

Both modes verify GitHub identity through `auth.openbitfun.com`, so signing in
requires internet access. An invitation contains only the selected endpoint
and device id (`/#/pair?did=<device-id>`); scanning it grants no authority.
The controller must sign in and resolve that id in its same-account directory.
Anonymous room pairing and tunnel-provider startup have been removed.

SSH and Docker workspace connections remain independent of Relay login.

## For community developers and forks

Self-hosting is supported through source and deployment scripts. The public mode uses a fixed official endpoint; it has no deployment wizard or editable
Relay URL. A private Relay therefore needs a matching client build.

1. Fork this workspace and read `CONTRIBUTING.md`. Build the Relay and mobile
   controller from the same revision. Keep your fork's changes in source control.
2. Select your HTTPS endpoint in `product-domains/src/account.rs`, then align
   the frontend constants in `src/web-ui/src/infrastructure/remote-connect/remoteConnectionState.ts`
   and `src/mobile-web/src/services/pairingLink.ts`. Native clients have
   matching constants in KMP `core-transport/AccountDeviceLink.kt` and HarmonyOS
   `services/AccountDeviceLink.ets`; update the HarmonyOS account-link parser too.
   Search for `https://remote.openbitfun.com/v/1.0.0` to verify every runtime
   reference and corresponding test before building your distribution.
3. Decide who owns identity. You can retain the official GitHub identity
   authority, or run the [shared identity service](../../../deploy/miniapp-market/README.md)
   with your own GitHub OAuth application. For an independent authority, change
   `IDENTITY_ME_URL` in `relay-service/src/identity.rs` and
   `DEFAULT_ACCOUNT_API_URL` in `services-integrations/src/account_identity/mod.rs`
   together, and adapt the market sign-in links and callback/completion host.
   `OPENBITFUN_ACCOUNT_API_URL` overrides the desktop/CLI identity API for
   development; the previous `OPENBITFUN_MINIAPP_MARKET_API_URL` alias remains
   readable. Relay never accepts an identity authority from a client request.
4. Use separate persistent data and asset directories, configure exact browser
   CORS origins, then put the service behind your own TLS reverse proxy. Build
   and exercise two devices using the same GitHub identity before distributing
   your fork. The public web controller must come from that matching build.

The scripts remain in this directory: `deploy.sh` deploys on the machine where
it runs, `common.sh` contains Docker/health helpers, and `mirror.sh` and
`release-download.sh` support mirrors and published images. Inspect
`bash deploy.sh --help` first. For fork code use
`bash deploy.sh --build-from-source --global-mirror`; the default image path
pulls a published upstream release, so it will not include your modifications.
An empty account database is normal: successful GitHub verification creates an
identity. Do not run retired `add-user` or password-reset commands.

The legacy script uses its own Compose project and defaults. For a fresh
versioned deployment, prefer the isolated [v1 Compose project](../../../deploy/relay-v1/README.md)
and adapt its host paths, bind port, proxy host, and trusted upstream ranges to
your infrastructure. Never reuse production data directories or an existing
container name for a development deployment. There is no need to restore the
removed deployment wizard to operate these scripts.

## Operator startup

This directory owns the official service binary and maintenance tools. The
shared HTTP/WebSocket implementation lives in `src/crates/services/relay-service`.

Set `RELAY_DB_PATH` to a persistent SQLite database before starting the service.
Startup fails if it is missing; anonymous public relay mode is unsupported.
The service validates OpenBitFun access tokens against the fixed GitHub identity
authority at `https://auth.openbitfun.com/api/v1/me`.

```bash
cargo build --release -p openbitfun-relay-server
RELAY_PORT=9700 RELAY_DB_PATH=/var/lib/openbitfun-relay-v1/relay.db \
  RELAY_ASSET_DIR=/var/lib/openbitfun-relay-v1/assets \
  ./target/release/openbitfun-relay-server
```

Use the isolated [v1 Compose project](../../../deploy/relay-v1/README.md).
Set `RELAY_LISTEN_ADDR=127.0.0.1:19700` with host networking so the service can
verify the immediate loopback proxy peer. Invalid listener values fail startup.
Expose only the TLS reverse proxy. Keep the database and asset paths distinct from older deployments.
`relay-admin` supports listing and explicitly deleting accounts; GitHub login
creates identities. Password provisioning, password reset, and user-entered
Relay server URLs are retired.

## Public-service resource controls

These limits protect the service independently of reverse-proxy configuration.
They are implemented in the shared Relay service, not in the agent loop.

| Resource | Limit and overload behavior |
|---|---|
| Authentication request body | 16 KiB; oversized bodies return 413 |
| Buffered HTTP request bodies | 512 MiB total reserved before buffering; overload returns 503 |
| Concurrent HTTP API requests | 2,048; overload returns 503 |
| Body read / device RPC handler | 15 seconds / 130 seconds |
| HTTP request rate | 6,000/minute per source IP; device APIs also per account; overload returns 429 |
| GitHub authorization start / poll | 10 / 120 per minute per IP |
| Identity exchanges | 10/minute per IP; 64 concurrent outbound identity requests |
| WebSocket upgrades | 120/minute per IP; 4,096 active sockets globally |
| WebSocket authentication | Must complete within 10 seconds |
| WebSocket ingress | 16 KiB per message/frame; 4 KiB read buffer per connection |
| WebSocket messages | 12,000/minute per connection |
| WebSocket outgoing queue | 128 messages per socket, 256 MiB total queued/writing bytes |
| Slow WebSocket writes | Close after a 15-second write timeout |
| RPC response memory | 256 MiB covering queued payloads and serialized replies, retained until read or disconnect |
| Pending device RPCs | 2,048 globally; 64 per account; cancellation releases capacity |
| Registered devices / active credentials | 64 / 256 per account; database-atomic admission |
| Device RPC ciphertext | 48 MiB, with JSON envelope allowance |

Existing devices can reconnect at the registration limit. Idempotent token
replays remain valid at the credential limit. Limits never delete a user's
session, device, workspace, or other product data.

Bearer authentication precedes body buffering on device APIs. Device discovery,
public-key lookup, message routing, and RPC correlation all enforce account
ownership. Delegated controller credentials cannot register sockets, mint more
credentials, or delete devices; revoking their parent device revokes them.
Account-enabled services reject the retired anonymous pairing-room endpoints.

The identity HTTP client rejects redirects, bounds response size and duration,
and never accepts a caller-provided identity authority. Browser CORS uses an
explicit origin list; wildcard CORS is rejected by the standalone host when
account APIs are enabled. Published Pages must use an origin separate from the
account sign-in surface. Without both isolated origins, the standalone host
returns 503 for `/api/pages`, `/api/page-auth`, and `/p` routes.

Application limits do not replace network-layer protection. Public deployment
also requires bounded proxy connections and request bodies, TLS, upstream DDoS
protection, and alerts for saturation, rejected requests, and disk growth.
Do not log Authorization headers, OAuth transaction secrets, tokens, request
bodies, or URL query strings containing sign-in state.

## Versioned reverse proxy

Strip only the new version prefix when forwarding. Do not replace the existing
`/relay` location. The proxy must overwrite forwarded IP headers with its own
observed source address, and the upstream port must be unreachable externally.
The Relay trusts forwarded client IPs only from an immediate loopback peer.

```nginx
location ^~ /v/1.0.0/ {
    proxy_pass http://127.0.0.1:19700/;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection $connection_upgrade;
    client_max_body_size 49m;
    client_body_timeout 15s;
    proxy_read_timeout 140s;
    proxy_send_timeout 30s;
}
```

Define the standard `$connection_upgrade` map and deployment-specific
`limit_req`/`limit_conn` zones in the owning Nginx configuration. Tune worker and
file-descriptor limits against measured concurrent sockets; daily active users
alone are not a capacity measurement. Preserve ordinary streaming and attachment
traffic in the load test when tuning rate limits.

Before opening the new location, exercise invalid/expired credentials,
cross-account access, concurrent quota exhaustion, oversized/slow bodies,
unauthenticated and slow-reader sockets, cancellation, reconnect, and normal
streaming. Verify that overload returns promptly and releases memory. Confirm
that the old service remains healthy and that rollback only removes the new
location and process.

## API

| Endpoint | Purpose |
|---|---|
| `GET /health`, `GET /api/info` | Health and service version |
| `POST /api/auth/github/start`, `/api/auth/github/poll` | Browser GitHub authorization |
| `POST /api/auth/login` | Exchange verified identity for a keyed device credential |
| `POST /api/auth/logout` | Revoke a credential |
| `POST /api/auth/delegate` | Issue a separately keyed, restricted controller credential |
| `POST /api/auth/provision-device` | Authorized SSH host bootstrap |
| `GET /api/devices` | Same-account device directory |
| `GET /api/devices/{id}/key` | Same-account device public key |
| `POST /api/devices/{id}/rpc` | Encrypted request/response forwarding |
| `POST /api/devices/{id}/messages` | Authenticated device responses and same-account messages |
| `DELETE /api/devices/{id}` | Explicit device removal and revocation |
| `GET /ws` | Authenticated device presence and encrypted messages |

`auth_connect` verifies a device token before WebSocket routing is enabled.
Devices receive requests over WebSocket and submit payloads through the HTTP
`messages` endpoint, which reserves memory before buffering. Correlation replies
must come from the expected account and device. Small legacy `device_message`
envelopes remain recognized; attachment-sized WebSocket ingress is rejected.
The versioned client and server must be deployed together for this transport.

## Configuration

`RELAY_PORT`, `RELAY_DB_PATH`, `RELAY_STATIC_DIR`, `RELAY_ROOM_WEB_DIR`,
`RELAY_ASSET_STORE_MAX_BYTES`, and `RELAY_CORS_ALLOW_ORIGINS` are operator
settings. `RELAY_PAGE_PUBLIC_BASE_URL` and `RELAY_PAGE_AUTH_BASE_URL` must be set
together and use distinct origins when protected Pages are deployed.

## Verification

```bash
cargo test -p openbitfun-relay-server --bin openbitfun-relay-server
cargo test -p openbitfun-relay-service
cargo check -p openbitfun-relay-server
node scripts/check-core-boundaries.mjs
```

Unit and integration tests are local evidence. Record live remote-control,
peer-device, remote-workspace, and detached-dispatch validation separately.
