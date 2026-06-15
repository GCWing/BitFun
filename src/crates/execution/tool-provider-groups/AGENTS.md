# tool-provider-groups Agent Guide

Scope: this guide applies to `src/crates/execution/tool-provider-groups`.

`bitfun-tool-packs` owns tool feature-group scaffold metadata, the product tool
provider group plan, and provider-group plan selection by id. It does not own
concrete tool implementations yet.

## Guardrails

- Keep `default = []`; `product-full` may aggregate feature groups but must not
  silently enable new runtime behavior. Boundary checks enforce the current
  feature-group list.
- Do not depend on `bitfun-core`, concrete service crates, app crates, Tauri,
  Git, MCP, network clients, or CLI UI dependencies unless a reviewed tool
  runtime owner move explicitly changes this boundary.
- Do not own manifest/exposure contracts, concrete runtime manifest assembly,
  `GetToolSpec` execution, collapsed unlock state, snapshot decoration, or
  `ToolUseContext`. Provider group plans may list group ids and tool names only.
- Product capability packs may select provider group ids; this crate owns the
  provider group plan and unknown provider-group validation.
- Future concrete tool migration must preserve product registry order,
  expanded/collapsed exposure, prompt stubs, unlock state, cancellation, runtime
  restrictions, and Deep Review tool flow.

## Verification

```bash
cargo test -p bitfun-tool-packs --features basic
cargo check -p bitfun-tool-packs --features product-full
node scripts/check-core-boundaries.mjs
```
