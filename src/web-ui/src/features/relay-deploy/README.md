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

16. **Scripts on the relay host are LF-only, in three independent layers.**
   `include_str!` and the `r#"..."#` remote templates both inherit the
   checkout's line endings, and Git for Windows checks out CRLF by default.
   Remote bash then runs the CR as a command and `set -euo pipefail` aborts on
   the first blank line (`deploy.sh: line 37: $'\r': command not found`).
   - `.gitattributes` pins LF, so the binary carries no CR.
   - `to_unix_script` normalizes everything sent over SFTP or `execute_command`,
     so a stale CRLF working tree still builds a working client.
   - `stage_scripts_command` strips CR **on the host, after upload and before
     the PTY runs the driver**. This is the layer that does not depend on the
     uploader remembering anything: a new `sftp_write` that forgets
     `to_unix_script` is still safe. Keep it that way — do not move the strip
     back to the client only.

17. **`sg -c` takes a single string, so quote every argument.** `sg docker -c
   "docker $*"` re-parses through a second shell and loses argument boundaries.
   Use `bitfun_shell_join` (`shell_join` in `common.sh`).

18. **`DOCKER_CONFIG` must be usable by whoever runs docker.** The Docker-install
   task runs as root with the SSH user's `HOME`, so it must hand `~/.bitfun`
   back to that user, and no `sudo` invocation may forward the user's
   `DOCKER_CONFIG` to root. A root-owned `config.json` makes the CLI warn and
   then mis-dispatch the build. Deploy repairs the config unconditionally — it
   cannot rely on `bitfun_resolve_docker_mode`, which is skipped when the driver
   already resolved a non-direct mode.

19. **Losing the runtime image build costs 20 minutes.** Retry it (clean Docker
   config, then classic builder) before falling back to a source rebuild.

19a. **The runtime base image's glibc must cover every archive a client might
   still install — not just what CI builds today.** arm64 releases through
   v0.2.14 were built on ubuntu-24.04-arm and require **GLIBC_2.38**; on
   `debian:bookworm-slim` (2.36) the relay could not load at all, and the deploy
   showed only a failed health check. The release matrix now pins both arches to
   ubuntu-22.04 (glibc 2.35, asserted by `scripts/ci/check-glibc-floor.sh`), but
   that does **not** make bookworm safe again: Desktop pins the release tag to
   its own version, so a v0.2.14 client installs the 2.38 archive forever. Base
   stays `debian:trixie-slim` (2.41). The image build also greps `ldd` output to
   fail fast on a mismatch — `ldd` exits 0 even while reporting an unsatisfied
   symbol version, so its *output* is the gate, and the relay binary is no use
   as a probe because it has no `--version` and just starts serving.

19b. **The source-build fallback must not redo the release path.**
   `bitfun_run_deploy_sh` is reached only after `bitfun_try_release_deploy`
   failed, and `deploy.sh` begins with that same release path — so it must be
   invoked with `--build-from-source`. Otherwise the published-binary attempt
   runs twice, visibly, before the source build.

19c. **Never send container diagnostics to /dev/null.** `docker logs` relays the
   container's stderr on its own stderr and the relay logs through tracing, so
   `2>/dev/null` discards exactly the line that explains the failure.

20. **Prepare-phase death must surface as failure.** The driver claims
   `<stem>.driver.pid` before anything that can fail. Poll keeps reporting
   `preparing` while that pid is alive — an open sudo prompt is unbounded — but
   a missing/dead driver past the grace window is `failed`, not perpetual
   "running". A dying driver writes to the PTY, not the log, so the log pane can
   be empty.

## Related docs

- Relay runtime / admin: [`src/apps/relay-server/README.md`](../../../apps/relay-server/README.md)
- Account login + sync choice: comments on `account_login` /
  `account_finalize_login` in `src/apps/desktop/src/api/remote_connect_api.rs`
