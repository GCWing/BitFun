# Configuration contracts

Pure persisted configuration records, defaults, validation and conversion to
shared model DTOs. No filesystem, process, network, Tauri or Core dependencies.
Keep existing schema versions and tolerant field defaults. Core retains the
live ConfigProvider interface and re-exports the records from its old paths.
The ts feature preserves TypeScript export capability without enabling runtime.

`normalization` owns pure model-field validation, capability normalization,
reference reconciliation, and persisted-value recovery shared by startup and
offline migration. Recovery retains unusable model records disabled and returns
diagnostics; callers own persistence. Catalog/provider-dependent reasoning
validation remains in the runtime configuration provider.
Keep 0.2.x model-selector translations, such as `auto` to `primary`, in the
legacy migration adapter. Shared recovery handles invalid current model
references without recognizing historical selector aliases.

Focused verification:

```bash
cargo test -p openbitfun-config-contracts --lib
```
