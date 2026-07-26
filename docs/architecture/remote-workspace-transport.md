# Remote workspace transport

This document defines how BitFun turns SSH hosts and Docker containers into one
workspace runtime without leaking transport-specific behavior into Agent,
search, terminal, or file-service callers.

## Goals

One saved target must have the same workspace semantics across:

- direct SSH and an arbitrary `ProxyJump` chain;
- a container reached through its own sshd;
- `docker exec` on a remote Docker host; and
- `docker exec` on the local machine.

Once connected, all workspace commands, terminal sessions, Agent subprocesses,
ACP processes, search helpers, and file operations target the effective
workspace. Docker-host commands are never an implicit fallback.

The transport adapters run on macOS, Windows, and Linux clients. Remote paths
remain POSIX paths on every client, and host `std::path` semantics must never be
used to split or join them. Docker execution targets a POSIX-compatible
container shell.

## Issue capability matrix

| Issue tier | Capability | Design owner |
|---|---|---|
| P0 | Arbitrary ProxyJump chain, per-hop host/user/key, staged errors | SSH session establishment |
| P0 | Local/remote Docker, direct container sshd, `docker exec` | Effective target resolution |
| P0 | Terminal, Agent, files, ACP, and search stay inside the container | Workspace stdio and file adapters |
| P1 | SSH config import and Docker container discovery | Existing remote connection dialog |
| P1 | sshd probe with `docker-exec` fallback | `container.access: auto` |
| P1 | Jump/target/container test stages | Connection test report |
| P2 | ssh-agent, OpenSSH certificates, keyboard-interactive challenges | SSH authentication adapter |
| P2 | Bounded connect/auth timeouts, retries, and challenge rounds | `SSHConnectionOptions` |
| P2 | TTY, stdin, long-running completion, interrupt/kill | Terminal adapter and `WorkspaceStdio` |
| Optional | Arbitrary host-side diagnosis from a container workspace | Deliberately excluded; requires a future typed read-only security surface |

## Configuration and runtime resolution

`SSHConnectionConfig` is the user-authored, persisted target. An
`ActiveConnection` retains that configuration for reconnect and drift
detection, and separately stores an `effective_config`.

For `container.access: auto`, connection establishment probes the container's
published `22/tcp` endpoint:

1. local Docker opens a normal SSH session to the published loopback port;
2. remote Docker opens a second SSH session through a `direct-tcpip` channel
   owned by the Docker-host session;
3. a successful handshake and authentication resolves the effective access to
   `sshd`;
4. an unavailable or rejected endpoint falls back to `docker-exec` and probes
   the configured container shell.

Runtime code only reads `effective_config`. The original `auto` value remains
persisted so a later reconnect can discover that sshd has become available.

## ProxyJump

The comma-separated jump chain is resolved left to right. Each token can be a
`~/.ssh/config` alias or `[user@]host[:port]`. Every hop has an independent
resolved host, port, user, identity file, certificate, and host-key check.
`direct-tcpip` channels carry the next SSH handshake; handles for all preceding
hops remain owned by the active connection.

Errors carry the stage name (`Jump N`, final target, or container sshd) and
separate reachability, handshake, and authentication failures. Connection and
authentication timeouts, whole-chain connection retries, and maximum
keyboard-interactive challenge rounds are bounded by `SSHConnectionOptions`.

Agent and OpenSSH certificate authentication are available on the target and
through SSH-configured jumps. Explicit password or keyboard-interactive
responses can be reused by a jump when that jump has no independent identity
configuration. Challenge responses and passphrases are runtime-only.

## Workspace stdio

`WorkspaceStdio` is the process-level port shared by SSH and local Docker:

```text
caller
  ├── stdin  ───────────────▶ workspace process
  ├── stdout ◀─────────────── workspace process
  ├── stderr ◀─────────────── workspace process
  ├── interrupt / kill ─────▶ supervisor
  └── completion / exit code ◀ supervisor
```

The SSH adapter pumps a `russh` channel. The local adapter supervises a piped
child process. Dropping all public IO streams cancels the owner, and explicit
interrupts escalate through the existing remote-exec grace period. This port is
used by:

- non-TTY remote execution, including stdin writes;
- local and remote Docker file streams;
- remote ACP subprocesses; and
- remote Flashgrep search helpers.

For non-TTY Docker processes, the command shell records the child PID inside
the container and uses `setsid` when available. Interrupt and kill requests
open a separate local or remote `docker exec` control path, signal the
in-container process group, and then close the owning Docker CLI transport.
This prevents cancelling the client-side CLI while leaving the workspace
process running. PID tracking is an enhancement, not a new execution
prerequisite: containers with a read-only temporary directory keep the legacy
Docker execution path and fall back to transport-level cancellation.

TTY execution remains a terminal-specific adapter: SSH requests a PTY, while
local Docker uses the existing local PTY service with `docker exec -it`.

## Files

SSH workspaces continue to use SFTP. Docker workspaces use binary stdio streams,
not text or base64 envelopes.

Reads stream chunks and report real byte progress. Writes stream to a unique
temporary file in the destination directory and rename it only after the input
has completed successfully. Cancellation kills the process and the shell trap
removes the temporary file, so an interrupted upload does not replace a valid
destination with partial content.

Directory and stat records use NUL-separated fields. File names containing
newlines or the delimiters used by older implementations remain round-trippable.
The records are decoded only after the full byte stream is assembled; invalid
UTF-8 names return an explicit unsupported-path error. Likewise, streamed text
output keeps incomplete UTF-8 suffixes between transport chunks instead of
inserting replacement characters at arbitrary chunk boundaries.

Remote names are validated before recursive download. Traversal components and
local-platform-invalid names are rejected, and case-colliding sibling names are
rejected on Windows and macOS before either entry can overwrite the other.
Recursive local uploads reject non-UTF-8 names and symbolic links explicitly;
recursive downloads reject remote symbolic links. Transfers never silently
omit an entry or follow a link outside the selected tree.

Host bind mounts are not path-translated. A host path is visible only at the
path mounted inside the container.

## Authentication and secrets

Supported target methods are password, private key, private key plus OpenSSH
certificate, OpenSSH agent, and keyboard-interactive responses.

- Passwords use the existing encrypted password vault.
- Private-key paths, certificate paths, and auth method metadata may be saved.
- Private-key passphrases, keyboard-interactive responses, and OTP values are
  never copied into `SavedConnection`.
- A saved interactive profile is retained but requires credential entry on the
  next manual connection.
- A legacy serialized `Agent` profile keeps its old
  `~/.ssh/id_rsa` compatibility fallback when the agent is unavailable.

## Product surface

The existing remote connection dialog owns all target types. It can import SSH
config hosts, discover local or remote Docker containers, choose `auto` or
`docker-exec`, and test the resolved jump/target/container stages before
connecting.

BitFun intentionally does not expose an arbitrary “run on Docker host” action
from a container workspace. That would bypass the selected workspace and its
security boundary. Host diagnosis, if added later, must be a typed, read-only
capability with a distinct confirmation and audit surface.

## Upgrade compatibility

New configuration fields have Serde defaults. Profiles written before this
design remain direct SSH targets with the same IDs, credentials, paths, and
workspace restore entries. Legacy port-bearing IDs are migrated together with
password-vault and workspace references.

Local Docker profiles may legitimately retain an empty legacy password
placeholder. Connection, testing, and local container discovery do not require
a password-vault entry for those profiles.

Startup recovery never deletes a profile or workspace merely because a
credential is unavailable, a connection times out, or a remote host is
temporarily offline. Destructive removal remains an explicit user action.

Contract tests cover legacy Agent/profile deserialization, defaulted connection
options, remote-workspace retention, stdio round trips, cancellation, and
delimiter-safe Docker metadata parsing. A Docker-backed ignored integration test
is available through `BITFUN_TEST_DOCKER_CONTAINER`.
