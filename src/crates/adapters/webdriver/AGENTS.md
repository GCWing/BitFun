# webdriver Agent Guide

Scope: this guide applies to `src/crates/adapters/webdriver`.

`bitfun-webdriver` owns the embedded desktop WebDriver bridge. It is a
platform-integration crate, not a product runtime or tool-policy owner.

## Guardrails

- Keep startup gated by the existing debug, feature, and environment checks.
- Platform capture, evaluation, and native WebView access may live here; product
  policy, session lifecycle, tool exposure, and agent decisions must not.
- Preserve WebDriver protocol response shapes, session/window/element semantics,
  and platform-specific capture/evaluation behavior.
- Do not expose this crate as a shared runtime contract; route product-facing
  behavior through desktop/API/transport boundaries.

## Verification

```bash
cargo check -p bitfun-webdriver
```

For documentation-only changes, run `git diff --check`.
