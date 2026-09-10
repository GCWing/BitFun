# Flashgrep binary distribution

Desktop development and packaging download the pinned release from
https://github.com/wgqqqqq/flashgrep-binaries/releases. No binaries belong in Git.
`VERSION.json` pins the repository, tag, filename, size, and SHA-256 for every target.

Run `node scripts/prepare-flashgrep-resource.mjs` for the current host, or pass
`--target x86_64-pc-windows-msvc` (including `--target=...` syntax). Desktop builds
use the explicit Tauri target, falling back to the Rust host triple. Linux GNU
application targets use the matching musl Flashgrep binary. Unsupported targets
fail explicitly. The preparation step requires curl (curl.exe on Windows).

Downloads go to ignored files in this directory. Cached files are verified before
reuse; failed or corrupted downloads never become package inputs. Only the
selected binary is added to Tauri resources, even if the cache holds other targets.
macOS release builds sign a copy with APPLE_SIGNING_IDENTITY, preserving the
original download and its checksum. The Windows custom installer currently targets
x86_64 and requires its matching Flashgrep payload.

Remote workspaces do not support Flashgrep. Index controls are hidden and remote
index/content/glob service calls return an explicit unsupported error before SSH
or local filesystem access. Remote file-name search and agent search paths that
already use remote shell tools remain available. Saved preferences are retained.

## Updating the pinned release

Publish the six standalone binaries and SHA256SUMS to the public binary repository
as Release assets, without publishing private source or MCP bundles. Verify the
release, then update VERSION.json with its tag and asset checksums/sizes. Do not use
`latest` or replace assets of an already consumed release; publish a new version.
OpenBitFun builds only need public download access, with no private-repository token.

## Focused verification

```sh
node --test scripts/prepare-flashgrep-resource.test.mjs scripts/desktop-tauri-build.test.mjs OpenBitFun-Installer/scripts/build-installer.test.cjs
```
