# One-click Relay Deploy

Desktop wizard that SSHes to a user-owned Linux host, installs Docker when it
is missing, pulls the signed OpenBitFun Relay image, and starts it. Account import
remains optional.

Entry points:

- Remote Connect → My OpenBitFun → login form → “一键部署到自己的服务器”
- Remote Connect → Network Relay → Self-Hosted → same action (must open this
  wizard, not an external README)

Backend orchestration:
`src/crates/services/services-integrations/src/remote_ssh/relay_deploy.rs`
Desktop Tauri surface: `src/apps/desktop/src/api/relay_deploy_api.rs`

## Invariants (do not regress)

1. **One click means install-if-needed, pull, start.** The deploy action must
   continue through Docker Engine installation in the same interactive task.
   Docker Compose, git, tar, Cargo, and a source checkout are not prerequisites.

2. **Customer servers never build Relay.** The normal Desktop path contains no
   archive extraction, `docker build`, repository sync, or source compilation,
   and it never silently falls back to those operations. Manual
   `deploy.sh --build-from-source` remains an explicit maintenance escape hatch.

3. **Authenticate the latest image before touching the server.** Desktop reads
   the latest `relay-image.json` and `relay-image.json.sig`, verifies the
   descriptor using its compiled-in minisign trust root, validates the exact
   repository, stable release tag/version, lowercase SHA256 digest, and both
   supported platforms, then sends only that trusted repository + digest to the
   remote script. The Desktop package version does not pin Relay deployment.

4. **Always start by digest.** Tags are discovery metadata, not an execution
   identity. One-click deploy sets `OPENBITFUN_REQUIRE_IMAGE_DIGEST=1`; Docker pulls
   and runs `<repository>@sha256:...`, so every manifest and layer remains
   content-addressed.

5. **Registry prefixes are transport, not trust roots.** Automatic mode keeps
   official GHCR first when a 10-second GitHub byte probe reaches 512 KiB/s; a
   slower or unreachable GitHub moves `ghcr.nju.edu.cn/...` and
   `m.daocloud.io/ghcr.io/...` ahead of official GHCR. Explicit global/China
   choices remain authoritative. Every route uses the same signed digest and
   has a bounded attempt before failover.

6. **The release must be publicly pullable.** `desktop-package.yml` builds one
   amd64/arm64 manifest, signs its descriptor, logs out of GHCR, and verifies an
   anonymous manifest read. A private package must fail publication rather than
   produce a green workflow customers cannot use.

7. **One implementation, two callers.** Pull, route fallback, container start,
   rollback, and health logic live in
   `src/apps/relay-server/release-download.sh`; `deploy.sh` sources it and
   `relay_deploy.rs` embeds it with `include_str!`. Do not fork that behavior
   back into a Rust string template.

8. **Preserve the container contract.** Keep container name `openbitfun-relay`,
   volumes `relay-server_relay-db` and `relay-server_room-web`, selected port,
   `/app/data`, `/app/room-web`, and `/app/relay-admin` stable across upgrades.

9. **Never stop a healthy Relay before the image is pulled.** Pull first, then
   rename the existing container, start the replacement, and remove the backup
   only after `/health` succeeds. Start, cancellation, or health failure must
   restore the previous container. Keep container stderr in failure diagnostics.

10. **Close wizard = cancel remote task.** Kill the detached body process tree.
    The image script's TERM/INT trap owns restoration; cancellation must not
    stop an unrelated healthy Relay or broad BuildKit/Compose processes.

11. **Account password never leaves this device.** Provision locally, then
    `relay-admin import-user` over the SSH session. Do not send plaintext
    passwords to the remote as env/script args.

12. **“Already deployed” is container-aware, not only selected-port health.**
    Changing the listen port must not hide a running `openbitfun-relay`. “Create
    account” must use the running container's actual published port.

13. **Port conflict ≠ our Relay.** `port_busy && !port_owned_by_relay` blocks
    deploy; busy-because-openbitfun-relay does not.

14. **Privilege handling stays interactive and minimal.** Never call `sudo -v`
    unconditionally. Detect root / passwordless sudo / interactive sudo. A
    missing Docker engine elevates once, installs through the selected regional
    route, repairs ownership, and continues without requiring a new login.

15. **`DOCKER_CONFIG` must remain usable by the SSH user.** Root installation
    keeps the user's HOME, so hand `~/.openbitfun` back before continuing. Repair or
    relocate an unreadable Docker config before any pull. Do not forward the
    user's config into an unrelated root home.

16. **Scripts on the Relay host are LF-only in three layers.** `.gitattributes`
    pins LF; `to_unix_script` normalizes generated uploads; and
    `stage_scripts_command` strips CR on the host before execution. Keep the
    host-side defense even when the client already normalized bytes.

17. **`sg -c` takes one string.** Quote every Docker argument with
    `openbitfun_shell_join` (`shell_join` in `common.sh`); never interpolate `$*`
    directly through the second shell.

18. **Prepare-phase death must surface as failure.** Keep reporting `preparing`
    while the driver PID is alive (a sudo prompt is unbounded), but treat a dead
    driver past the grace window as failed rather than running forever.

19. **The runtime image keeps the compatibility gate.** `Dockerfile.release`
    uses `debian:trixie-slim` and inspects `ldd` output for both binaries. This
    covers older arm64 Relay artifacts that required GLIBC 2.38 even though the
    current release builders assert a GLIBC 2.35 ceiling.

20. **Release metadata has a byte mirror, not a second authority.**
    `scripts/openbitfun-release-sync.sh` mirrors the signed descriptor into both its
    version directory and `/release/relay-image.json`. Desktop may fetch those
    bytes when GitHub is unreachable, but the same built-in minisign key must
    verify them.

## Related docs

- Relay runtime / admin: [`src/apps/relay-server/README.md`](../../../apps/relay-server/README.md)
- Account login + sync choice: comments on `account_login` /
  `account_finalize_login` in `src/apps/desktop/src/api/remote_connect_api.rs`
