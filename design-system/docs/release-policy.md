# Release policy

The first public design-system packages use a fixed release group:

- `@openbitfun/design-tokens`
- `@openbitfun/theme-openbitfun`
- `@openbitfun/ui`

During the initial contract-forming period they share the same version so a consumer can reason about compatibility without a matrix.

## Version meaning

- Patch: behavior-preserving implementation or documentation correction.
- Minor: additive token, component, variant, or theme capability.
- Major: removed or renamed token, incompatible component API, changed component anatomy, or a changed theme contract.

Theme value changes are always recorded even when the TypeScript API is unchanged.

## Publication gate

1. Build every public package from its own package script.
2. Run token graph, contrast, package-boundary, and public-registry tests.
3. Build Design Lab without source aliases so it consumes public package exports.
4. Pack each public package and verify the tarball contains only declared artifacts.
5. Publish from CI with provenance after registry credentials and the final package scope are configured.
