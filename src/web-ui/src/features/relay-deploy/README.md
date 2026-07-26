# One-click Relay Deploy

Desktop wizard that SSHes to a user-owned Linux host and deploys the matching
published Relay binary in a lightweight Docker image, with the source Docker
build retained as an automatic fallback. Account import remains optional.

Entry points:

- Remote Connect → My BitFun → login form → “一键部署到自己的服务器”
- Remote Connect → Network Relay → Self-Hosted → same action (must open this
  wizard, not an external README)

Backend orchestration:
`src/crates/services/services-integrations/src/remote_ssh/relay_deploy.rs`
Desktop Tauri surface: `src/apps/desktop/src/api/relay_deploy_api.rs`

## Invariants (do not regress)

1. **Published binary first, source fallback.** Download the matching stable
   `v<desktop-version>` asset or the `nightly` asset for nightly Desktop builds.
   Verify its `.sha256` and preserve the existing `bitfun-relay` container,
   volumes, ports, and `/app/relay-admin` contract. A download, checksum,
   runtime-image, startup, or health failure restores the previous container
   before falling back to source.

2. **Rank sources by measured speed; never by fixed priority.** The CN proxy,
   GitHub, and the openbitfun.com mirror each get a short ranged probe and the
   download goes to the fastest. A source that is slow rather than broken must
   not hold the deploy: `--speed-limit`/`--speed-time` abandons a dead link
   quickly, `-C -` resumes instead of restarting (a wall-clock ceiling alone
   made a 20 KB/s link retry from zero forever), and every source is tried
   fastest-first before giving up. If nothing clears the healthy-throughput
   bar, still download — a slow transfer beats a 20-minute source rebuild.

3. **Take the mirror URL from the mirror's own manifest**
   (`openbitfun.com/release/linux-binaries.json`), never a constructed
   `/<version>/` path. The mirror retains only the most recent releases, so a
   pinned version 404s for every older Desktop build.

4. **Verify the checksum's signature on this device, not the server.** A relay
   host is an arbitrary user machine with no minisign and no trust root, so it
   cannot check a signature. It does not need to: the release signs the
   `.sha256` file too, Desktop verifies that signature locally (a couple of
   hundred bytes) and exports the resulting hash into the generated script as
   `BITFUN_EXPECTED_SHA256_<TARGET>`. The remote then needs only `sha256sum`,
   and no origin can override that hash. Requires `BITFUN_RELEASE_PUBKEY` at
   Desktop build time.

5. **Without a verified hash, bind to a checksum from a different origin than
   the bytes.** A `.sha256` served by whoever served the archive only detects
   corruption; the CN path deliberately prefers a third-party GitHub proxy, so
   the checksum is fetched from the canonical GitHub URL (derivable from any
   candidate URL, including the mirror's versioned path). Same-origin fallback
   is allowed only when GitHub is unreachable, and must say so in the log.

6. **One implementation, two callers.** The download, verification and runtime
   image live in `src/apps/relay-server/release-download.sh`; `deploy.sh`
   sources it and `relay_deploy.rs` embeds it with `include_str!`, exactly as
   `mirror.sh` is shared. Do not fork this logic back into the Rust template —
   manual and one-click deploys must not drift.

7. **Fallback source path is `~/.bitfun/relay-src`**, never `$HOME/BitFun` /
   `$HOME/bitfun`. Sync always passes an explicit clone destination. Destructive
   replace is only safe under `~/.bitfun/`.

8. **Git first, tarball fallback.** When `.git` already exists, deploy must
   `fetch` + checkout, not re-clone from scratch (preserves BuildKit layers
   and Cargo cache mounts for registry/git/`target`).

9. **Close wizard = cancel remote task.** Do not leave nohup builds running
   after the modal closes; cancel must kill the pid tree and best-effort stop
   compose/buildx workers.

10. **Account password never leaves this device.** Provision locally, then
   `relay-admin import-user` over the SSH session. Do not send plaintext
   passwords to the remote as env/script args.

11. **“Already deployed” is container-aware, not only selected-port health.**
   Changing the listen port must not hide a running `bitfun-relay`. Use
   `container_running` / `existing_relay_port` / `relay_healthy` (health on
   selected **or** existing port). “Create account” must hit the running port.

12. **Port conflict ≠ our relay.** `port_busy && !port_owned_by_relay` blocks
   deploy; busy-because-bitfun-relay does not.

13. **Privilege / Docker install.** Do not call `sudo -v` unconditionally.
   Detect root / passwordless sudo / interactive elevate. Docker install must
   not require a working daemon *before* install.

14. **Scripts are embedded Rust templates** staged via SFTP. Do not rely on a
   static repo `.sh` alone on the server until the desktop binary re-stages.

15. **China mirrors before overseas downloads.** Desktop orchestration embeds
   `src/apps/relay-server/mirror.sh` and runs `bitfun_mirror_init` before apt
   tool install, Docker Engine install, and GitHub sync. `deploy.sh` sources
   the same file so manual and one-click paths stay aligned. Force with
   `BITFUN_MIRROR=cn|global`. Docker daemon metadata must stay outside
   `daemon.json`; host Cargo config must remain untouched; global mode rolls
   back only BitFun-managed apt and Docker entries.

## Related docs

- Relay runtime / admin: [`src/apps/relay-server/README.md`](../../../apps/relay-server/README.md)
- Account login + sync choice: comments on `account_login` /
  `account_finalize_login` in `src/apps/desktop/src/api/remote_connect_api.rs`
