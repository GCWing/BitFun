# One-click Relay Deploy

Desktop wizard that SSHes to a user-owned Linux host and deploys
`src/apps/relay-server` (Docker compose + optional first-account import).

Entry points:

- Remote Connect → My Devices → login form → “一键部署到自己的服务器”
- Remote Connect → Network Relay → Self-Hosted → same action (must open this
  wizard, not an external README)

Backend orchestration:
`src/crates/services/services-integrations/src/remote_ssh/relay_deploy.rs`
Desktop Tauri surface: `src/apps/desktop/src/api/relay_deploy_api.rs`

## Invariants (do not regress)

1. **Source path is `~/.bitfun/relay-src`**, never `$HOME/BitFun` / `$HOME/bitfun`.
   Sync always passes an explicit clone destination. Destructive replace is only
   safe under `~/.bitfun/`.

2. **Git first, tarball fallback.** When `.git` already exists, deploy must
   `fetch` + checkout, not re-clone from scratch (preserves BuildKit layers).

3. **Close wizard = cancel remote task.** Do not leave nohup builds running
   after the modal closes; cancel must kill the pid tree and best-effort stop
   compose/buildx workers.

4. **Account password never leaves this device.** Provision locally, then
   `relay-admin import-user` over the SSH session. Do not send plaintext
   passwords to the remote as env/script args.

5. **“Already deployed” is container-aware, not only selected-port health.**
   Changing the listen port must not hide a running `bitfun-relay`. Use
   `container_running` / `existing_relay_port` / `relay_healthy` (health on
   selected **or** existing port). “Create account” must hit the running port.

6. **Port conflict ≠ our relay.** `port_busy && !port_owned_by_relay` blocks
   deploy; busy-because-bitfun-relay does not.

7. **Privilege / Docker install.** Do not call `sudo -v` unconditionally.
   Detect root / passwordless sudo / interactive elevate. Docker install must
   not require a working daemon *before* install.

8. **Scripts are embedded Rust templates** staged via SFTP. Do not rely on a
   static repo `.sh` alone on the server until the desktop binary re-stages.

## Related docs

- Relay runtime / admin: [`src/apps/relay-server/README.md`](../../../apps/relay-server/README.md)
- Account login + sync choice: comments on `account_login` /
  `account_finalize_login` in `src/apps/desktop/src/api/remote_connect_api.rs`
