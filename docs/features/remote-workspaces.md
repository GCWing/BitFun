# Remote SSH and container workspaces

BitFun remote workspaces use one saved target for the file explorer, terminal,
Agent commands, and workspace tools. The target can be:

- an SSH host;
- an SSH host reached through one or more jump hosts;
- a Docker container on an SSH host;
- a Docker container on the local machine; or
- an sshd endpoint running inside a container.

## Jump hosts

`ProxyJump` accepts a comma-separated chain such as `jump1,jump2` or
`ops@jump.example.com:2222`. SSH config aliases are resolved from
`~/.ssh/config`. Each alias may provide its own `HostName`, `Port`, `User`, and
`IdentityFile`, so hop credentials do not need to match the final target.

BitFun opens each hop in order and carries the next SSH handshake over a
`direct-tcpip` channel. Connection errors identify the failed jump number or
the final target, and distinguish reachability from SSH authentication.

Password authentication remains available for the final target. Jump hosts in
the P0 flow use identity files from SSH config.

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

## Filesystem semantics

When a Docker target is selected:

- terminal sessions start in the container;
- Agent and task commands execute in the container;
- reads, writes, directory listings, rename, create, and delete operations
  address the container filesystem;
- the workspace path is a path inside the container, not a host path.

A host bind mount is visible only through the path at which it is mounted in
the container. BitFun does not silently translate host paths to container
paths. File transfer uses container commands and base64 for binary-safe file
content; ordinary SSH workspaces continue to use SFTP.

The configured Docker CLI remains the security boundary. BitFun does not expose
the Docker daemon over the network or bypass the current user's Docker
permissions.

## Upgrade compatibility

Existing SSH profiles remain plain SSH targets because the new `proxyJump` and
`container` fields are optional. Existing remote-workspace records keep their
paths and connection metadata. Legacy connection IDs that included the SSH port
are migrated together with their workspace references.

If a saved password is unavailable after an upgrade or local keychain reset,
BitFun keeps the connection and workspace records and asks for the password on
the next manual reconnect. A startup timeout or temporary network failure marks
the workspace as unavailable but does not delete its restore metadata.
