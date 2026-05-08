Place the prebuilt `flashgrep` daemon binary in this directory.

Expected filenames:

- macOS x86_64: `flashgrep-x86_64-apple-darwin`
- macOS arm64: `flashgrep-aarch64-apple-darwin`
- Linux x86_64: `flashgrep-x86_64-unknown-linux-gnu`
- Linux arm64: `flashgrep-aarch64-unknown-linux-gnu`
- Windows x86_64: `flashgrep-x86_64-pc-windows-msvc.exe`
- Windows arm64: `flashgrep-aarch64-pc-windows-msvc.exe`

BitFun dev/build scripts load the daemon from this repository-relative path.
