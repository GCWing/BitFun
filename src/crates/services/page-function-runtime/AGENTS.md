# Page Function Runtime

This crate owns the embedded JavaScript runtime used to execute BitFun Page
Functions (rquickjs).

## Ownership

- JS sandbox construction, function invocation, and structured result/error
  mapping belong here.
- Product assembly, relay HTTP routes, and host lifecycle stay outside this crate.
- Keep the runtime free of product capability selection and UI concerns.

## Boundaries

- Do not depend on assembly, interface, or application crates.
- Callers such as `relay-service` may depend on this crate for execution only.

## Verification

Run `cargo test -p bitfun-page-function-runtime` and
`node scripts/check-core-boundaries.mjs` after changes.
