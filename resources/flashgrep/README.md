Flashgrep distribution is temporarily suspended. The platform binaries have been removed,
and desktop development, packaging, and the Windows installer no longer require them.
The Web UI hides accelerated-search settings and index controls, including on remote
workspaces and peer devices. Saved preferences and backend implementations are retained.

To restore the feature, restore the binaries and desktop preparation/bundling steps,
then enable `WORKSPACE_SEARCH_AVAILABLE` in the Web UI.

Pinned release:

- `v0.2.16` from `wgqqqqq/flashgrep`

Expected filenames (restoration reference):

- macOS x86_64: `flashgrep-x86_64-apple-darwin`
- macOS arm64: `flashgrep-aarch64-apple-darwin`
- Linux x86_64: `flashgrep-x86_64-unknown-linux-musl`
- Linux arm64: `flashgrep-aarch64-unknown-linux-musl`
- Windows x86_64: `flashgrep-x86_64-pc-windows-msvc.exe`
- Windows arm64: `flashgrep-aarch64-pc-windows-msvc.exe`

macOS binaries are ad-hoc signed after download so local development can execute them directly.
