[中文](AGENTS-CN.md) | **English**

# Built-in Agent Content

This crate owns only immutable built-in Agent prompt bytes shipped with the
product and the stable lookup keys retained for `openbitfun-core` compatibility.
It has no third-party dependencies and is activated only by Core's
`product-full` assembly.

## Boundary Rules

- Keep prompt selection, rendering, mode policy, Memory and Insights workflow,
  runtime state, and error handling in their existing owners.
- Do not load user, project, product-customization, or plugin content here.
- Do not add a generic registry, provider lifecycle, runtime file lookup, file
  watching, or fallback path. Debug and release builds remain compile-time
  embedded and self-contained.
- Preserve the existing prompt keys and exact returned bytes. The generated
  Agent catalog intentionally retains the previous generated-Rust newline
  normalization; the Memory phase-1 and Insights direct constants retain their
  previous `include_str!` behavior.
- Keep the crate feature-free and dependency-free unless a concrete product
  requirement proves that this ownership boundary must change.

## Verification

Run `cargo test -p openbitfun-agent-content --test prompt_catalog_contracts` after
changing the catalog, lookup behavior, or prompt paths. Prompt text changes may
intentionally alter content, but must not silently remove or rename a stable key.
