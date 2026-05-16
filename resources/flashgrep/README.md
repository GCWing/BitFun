Place the prebuilt `flashgrep` daemon binary in this directory.

Pinned release:

- `v0.2.6` from `wgqqqqq/flashgrep`

Expected filenames:

- macOS x86_64: `flashgrep-x86_64-apple-darwin`
- macOS arm64: `flashgrep-aarch64-apple-darwin`
- Linux x86_64: `flashgrep-x86_64-unknown-linux-musl`
- Linux arm64: `flashgrep-aarch64-unknown-linux-musl`
- Windows x86_64: `flashgrep-x86_64-pc-windows-msvc.exe`
- Windows arm64: `flashgrep-aarch64-pc-windows-msvc.exe`

macOS binaries are ad-hoc signed after download so local development can execute them directly.

BitFun dev/build scripts load the daemon from this repository-relative path.
