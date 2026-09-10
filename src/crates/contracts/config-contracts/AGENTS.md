# Configuration contracts

Pure persisted configuration records, defaults, validation and conversion to
shared model DTOs. No filesystem, process, network, Tauri or Core dependencies.
Keep existing schema versions and tolerant field defaults. Core retains the
live ConfigProvider interface and re-exports the records from its old paths.
The ts feature preserves TypeScript export capability without enabling runtime.

Focused verification:

```bash
cargo test -p openbitfun-config-contracts --lib
```
