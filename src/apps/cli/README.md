# BitFun CLI

Terminal UI for BitFun (chat, tools, `/login` account + Peer Host).

## One-click install (Linux / macOS, amd64 + arm64)

From the repository root:

```bash
bash src/apps/cli/install.sh
```

Or from this directory:

```bash
bash install.sh
```

The script will:

1. `cargo build -p bitfun-cli --release` (native host CPU)
2. Install `bitfun-cli` to `~/.local/bin` (override with `BITFUN_CLI_BIN_DIR`)
3. Idempotently add a PATH block to `~/.bashrc` and `~/.zshrc`
4. `source` the matching rc when the current shell is interactive bash/zsh

Then run:

```bash
bitfun-cli
```

### Options / environment

| Variable | Meaning |
|----------|---------|
| `BITFUN_CLI_BIN_DIR` | Install directory (default `~/.local/bin`) |
| `BITFUN_CLI_SKIP_SHELLRC` | Set `1` to skip bashrc/zshrc edits |
| `CARGO_TARGET_DIR` | Cargo target dir (e.g. `$HOME/bitfun-build/target` on shared mounts) |
| `CARGO_BUILD_JOBS` | Limit rustc parallelism on small VPS |

Example on a small arm64 VPS:

```bash
CARGO_BUILD_JOBS=1 bash src/apps/cli/install.sh
```

### Prerequisites

- Rust toolchain (`rustup` / `cargo`)
- Repository checked out with workspace `Cargo.toml` at the root

## Dev commands (from repo root)

```bash
pnpm run cli:dev      # cargo run
pnpm run cli:build    # cargo build --release
pnpm run cli:install  # same as bash src/apps/cli/install.sh
```
