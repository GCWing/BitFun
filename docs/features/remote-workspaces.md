# Remote SSH and container workspaces

BitFun remote workspaces use one saved target for the file explorer, terminal,
Agent commands, and workspace tools. The target can be:

- an SSH host;
- an SSH host reached through one or more jump hosts;
- a Docker container on an SSH host;
- a Docker container on the local machine; or
- an sshd endpoint running inside a container.

The local client behavior is supported on macOS, Windows, and Linux. Remote
workspace paths are always interpreted with POSIX `/` separators, independent
of the client OS. Docker workspace commands require a POSIX-compatible
container shell; selecting a Windows container does not silently reinterpret
paths or commands with Windows semantics.

## Jump hosts

`ProxyJump` accepts a comma-separated chain such as `jump1,jump2` or
`ops@jump.example.com:2222`. SSH config aliases are resolved from
`~/.ssh/config`. Each alias may provide its own `HostName`, `Port`, `User`, and
`IdentityFile`, so hop credentials do not need to match the final target.

BitFun opens each hop in order and carries the next SSH handshake over a
`direct-tcpip` channel. Connection errors identify the failed jump number or
the final target, and distinguish reachability from SSH authentication.

Each SSH-configured jump may use its own identity, OpenSSH certificate, or
ssh-agent identity. Explicit password and keyboard-interactive challenge
responses are also supported, with configurable connection/authentication
timeouts, whole-chain retries, and challenge-round limits.

## Docker targets

For **Docker on SSH host**, BitFun first establishes the SSH connection (and
optional jump chain), then wraps workspace operations with:

```text
docker exec -i [--user USER] CONTAINER SHELL -lc COMMAND
```

For **Local Docker container**, the same command runs through the local Docker
CLI without opening SSH. The Docker executable, container user, and container
shell are configurable.

For **Container sshd**, the normal host, port, user, and authentication fields
must point directly to the container's sshd endpoint. Optional jump hosts use
the same SSH path described above.

`Auto` probes the container's published `22/tcp` endpoint and completes an SSH
handshake. If sshd is unavailable or rejects authentication, BitFun falls back
to `docker exec`. The connection dialog shows the resolved access mode and can
test jumps, the target, and the container before connecting. It can also list
containers from local Docker or from the configured SSH Docker host.

## Filesystem semantics

When a Docker target is selected:

- terminal sessions start in the container;
- Agent and task commands execute in the container;
- reads, writes, directory listings, rename, create, and delete operations
  address the container filesystem;
- the workspace path is a path inside the container, not a host path.

A host bind mount is visible only through the path at which it is mounted in
the container. BitFun does not silently translate host paths to container
paths. File transfer uses binary stdin/stdout streams. Uploads write to a
same-directory temporary file and rename atomically after success; cancellation
leaves the previous destination intact. Ordinary SSH workspaces continue to use
SFTP.

Text command output is decoded as one UTF-8 byte stream, so a multibyte
character split across SSH or Docker chunks is preserved. File bytes are never
decoded. Workspace metadata and paths must be valid UTF-8; names that cannot be
represented safely on the local filesystem (for example Windows reserved
names, traversal components, or case-colliding names on common Windows/macOS
filesystems) fail with an explicit error instead of being skipped or
overwritten. Recursive transfers reject symbolic links instead of following
them outside the selected tree.

Non-TTY Docker commands run under an in-container supervisor. Interrupt and
timeout handling signal the command's process group inside the container before
closing the local or SSH-hosted Docker CLI process. A read-only container
temporary directory does not make an existing Docker workspace unusable;
execution continues with transport-level cancellation as the compatibility
fallback.

The configured Docker CLI remains the security boundary. BitFun does not expose
the Docker daemon over the network or bypass the current user's Docker
permissions.

## Search on hosts without ripgrep

Agent Grep keeps one matching and result-processing implementation. For
case-sensitive literals and literal alternatives such as `foo|bar`, an available
compatible `rg` can preselect candidate files to reduce SSH transfer. Without
`rg`, BitFun can automatically use a compatible system `grep` in batches for the
same purpose. Both are checked for required behavior before use. The shared
scanner applies the query, file types, context, counting and pagination.
Complex regular expressions, case folding or unavailable target accelerators
use file streams through the existing workspace connection. Installing BitFun
on the target is not required. Stream scanning can transfer more data and take
longer on a slow connection; results report the backend and scanned bytes.

System grep is not a transparent replacement: its default regular expression
syntax, supported options and filename framing differ. BitFun does not silently
substitute a weaker expression and return a misleading empty result. Search
failures retain their diagnostics. Remote Glob uses the shared matcher on
POSIX paths, preserving filename boundaries and applying the pattern before
the result limit.

The built-in Grep tool accepts structured search arguments. This differs from
`ExecCommand`, where the model supplies a shell command executed in the Session's
target environment. A missing shell program is reported as such; BitFun does
not silently rewrite that command or install software.

## Session forks and file previews

Forking a remote session copies the selected conversation history and keeps the
source session's SSH connection and POSIX workspace path. It does not create a
Git worktree or copy remote files. The original and forked sessions continue to
use the same remote working tree. Fork storage is selected by the verified
remote binding, even when another host has a workspace with the same path.

Read, Write, Edit, Delete and LS share their tool logic across local and remote
workspaces. Only filesystem IO changes provider. Agent Runtime, credentials,
permissions, Session history and snapshot metadata stay on the BitFun host;
SSH workspaces do not start a remote BitFun CLI or shared daemon.

New remote file modifications can record snapshots in the host's local mirror,
isolated by the complete connection identity. A successfully recorded operation
can display its persisted summary and diff even after disconnecting. An older
operation, failed snapshot, or unsupported symbolic-link target does not claim
that history exists; tool cards retain their inline preview instead.
Forked conversation history retains inline tool results but does not inherit
the source Session's snapshot preview capability.

These operation records do not prove complete historical coverage. Full file
rollback and edit-and-rerun remain unavailable for remote Sessions until the
Session's coverage can be verified. Existing history is retained, and no
baseline is fabricated from current remote files. A snapshot bookkeeping
failure never causes a file tool to execute twice.

## Git ownership trust

Git refuses a repository whose directory is owned by another user until the
path is listed in the protected `safe.directory` configuration — a common shape
for shared SSH hosts, bind-mounted trees, and containers that run as a
different uid. BitFun classifies that refusal as its own state instead of
reporting "not a repository", so Git-backed surfaces can explain the wall and
the Review launch fails with a specific reason.

The decision itself belongs to the machine that owns the repository. For a
remote workspace BitFun reports the state and hands over the exact command
(`git config --global --add safe.directory "<path>"`) to run on the remote
host; it never writes the remote user's global Git configuration, and it never
grants trust implicitly as a fallback of a failed read. The same rule applies
in Peer Device Mode: the read-only probe answers for a controller, granting is
refused on the peer host.

## Upgrade compatibility

Existing SSH profiles remain plain SSH targets because the new `proxyJump` and
`container` fields are optional and connection policy fields have defaults.
Existing remote-workspace records keep their paths and connection metadata.
Legacy connection IDs that included the SSH port are migrated together with
their password-vault and workspace references.

Legacy local-Docker profiles do not need an SSH password-vault entry, even if
their old serialized auth placeholder is an empty password.

If a saved password is unavailable after an upgrade or local keychain reset,
BitFun keeps the connection and workspace records and asks for the password on
the next manual reconnect. A startup timeout or temporary network failure marks
the workspace as unavailable but does not delete its restore metadata.

Private-key passphrases, keyboard-interactive responses, and one-time codes are
never persisted. Saved interactive profiles therefore remain visible after an
upgrade but require manual credential entry before reconnecting.

For the ownership and transport contracts behind these behaviors, see
[`remote-workspace-transport.md`](../architecture/remote-workspace-transport.md).
