# Theme and color-token operations

> Companion to root `AGENTS.md` (STD-04 / STD-11) and
> [`docs/architecture/theme-token-optimization.md`](../architecture/theme-token-optimization.md).
> Architecture owns token ownership and layering; this page keeps the repository
> operational ratchet rules that used to live in root `AGENTS.md`.

- Theme and color-token baselines are ratchet contracts, not editable test
  expectations. Do not make a failing theme audit pass by raising values in
  `scripts/theme-color-governance-baseline*.json`, loosening fixture/assertion
  counts, adding broad allowlist entries, or removing CI audit coverage.
- Lower theme baselines when measured debt is removed. If a change truly needs a
  new color or key, add the smallest owner contract and document why existing
  semantic, component, or specialized-domain tokens cannot cover it.
- For theme, CSS variable, widget payload, mobile, installer, or CLI/TUI color
  changes, run `pnpm run theme:color-audit:all`.
