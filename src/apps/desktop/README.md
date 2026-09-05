# OpenBitFun Desktop

For development commands, see [AGENTS.md](AGENTS.md) and the repository
[contribution guide](../../../CONTRIBUTING.md).

## Application updates

Choose **Background download** in the new-version dialog to download and verify
an update while continuing to use OpenBitFun. Downloading does not install the
update or restart the application.

When the download finishes, OpenBitFun offers **Install and restart** or
**Later**. Installation restarts OpenBitFun on this device and interrupts its
active sessions. Choosing Later, or closing the dialog, keeps the downloaded
update. Open **About → Install and restart** whenever you are ready; the same
confirmation appears before installation.

Downloaded updates remain available after closing and reopening OpenBitFun.
After reopening, the current updater requires access to the update server to
restore installer metadata, but does not download the package again. If this
step or installation fails, the pending update remains available to retry.
Use **Download again** in the error dialog if the cached package is damaged.

Application updates always belong to the local desktop, including while viewing
a peer device or a remote workspace. They do not install software on the peer or
cancel independently running detached jobs on another host. Connections through
the restarting desktop are interrupted.

## Windows WSL workspaces

In the remote connection dialog, choose **Workspace target → Windows WSL**.
Select an installed Linux distribution, optionally enter a Linux user, then
connect and choose a folder inside that distribution. **Refresh distributions**
reloads the list after you install another distribution. No SSH server or SSH
credentials are needed. WSL must already be installed and the distribution must
have completed its first-run setup on the Windows host.

Files, search, Git, Agent commands, and terminal sessions use the selected Linux
filesystem and processes. Paths use POSIX separators. Saved connections retain
the distribution and optional user for reconnect; omitting the user uses the
distribution's configured default user.

In Peer Device Mode, the selected Windows Desktop host owns WSL discovery and
execution. Older peers and CLI peers explicitly refuse native WSL setup;
non-Windows hosts show an unsupported state. Existing mobile and bot session
controls can continue driving a host session, but do not expose WSL connection
setup. Detached Dispatch does not provision WSL connections. SSH port forwarding
is unavailable for native WSL targets; use Windows WSL networking to reach a
Linux service.
