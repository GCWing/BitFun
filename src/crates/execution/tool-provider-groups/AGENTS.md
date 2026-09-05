# tool-provider-groups Agent Guide

Scope: this guide applies to `src/crates/execution/tool-provider-groups`.

`openbitfun-tool-packs` owns tool feature-group metadata, the stable tool-to-feature
mapping, the product tool provider group plan, and provider-group plan selection
by id. Concrete implementations and runtime materialization remain in Core.

## Guardrails

- Keep `default = []`; `product-full` may aggregate feature groups but must not
  silently enable new runtime behavior. Adding a built-in tool requires one
  exact feature-group owner and boundary coverage for the full group list.
- Do not depend on `openbitfun-core`, concrete service crates, app crates, Tauri,
  Git, MCP, network clients, or CLI UI dependencies unless a reviewed tool
  runtime owner move explicitly changes this boundary.
- Do not own manifest/exposure contracts, concrete runtime manifest assembly,
  `GetToolSpec` execution, collapsed unlock state, snapshot decoration, or
  `ToolUseContext`. Provider group plans may list group ids and tool names only.
  A provider may contain tools from multiple feature owners; the exact
  tool-to-`ToolPackFeatureGroup` mapping is the owner authority.
  Compile-time availability is a validation fact; materialization must fail
  closed when a product plan requests a group that the binary did not compile.
  It must not infer the requested plan from compiled features or inspect runtime
  state, permissions, or service health. Filtering tools that belong to owners
  omitted by an explicit `ProductToolPlan` is product selection, not degradation.
- Product capability packs may select provider group ids; this crate owns the
  provider group plan and unknown provider-group validation.
- Future concrete tool migration must preserve product registry order,
  expanded/collapsed exposure, prompt stubs, unlock state, cancellation, runtime
  restrictions, and Deep Review tool flow.

## Verification

```bash
cargo test -p openbitfun-tool-packs --features basic
cargo check -p openbitfun-tool-packs --features product-full
node scripts/check-core-boundaries.mjs
```
