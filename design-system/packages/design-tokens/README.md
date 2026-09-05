# @openbitfun/design-tokens

Framework-neutral token contract and system scales for OpenBitFun UI packages.

This package intentionally contains no concrete brand palette. Install a theme package such as `@openbitfun/theme-openbitfun` alongside it.

```ts
import {
  cssVariables,
  tokenCatalog,
  tokenModes,
  tokens,
} from "@openbitfun/design-tokens";
import "@openbitfun/design-tokens/tokens.css";
```

Only semantic and system token names are public API. Density modes reuse the same names and override values through a scoped `data-density` attribute.

`tokenCatalog` is the authoring contract for visual tools. Each entry exposes its type, CSS variable, category, description, and resolved value in every density mode. Applications normally consume `tokens`; editors consume the catalog instead of maintaining a second token list.

## Semantic typography roles

Text-bearing components consume a complete semantic role rather than assembling
font family, size, weight, line height, and letter spacing from foundation
tokens. The core interface roles are:

| Content purpose | Token role | Default contract |
| --- | --- | --- |
| Page title | `type.heading.page` | Control, 24px, 700, 1.2, normal |
| Compact page title | `type.heading.compactPage` | Control, 20px, 600, 1.2, normal |
| Navigation title | `type.heading.navigation` | Control, 17px, 600, 1.2, normal |
| Section title | `type.heading.section` | Control, 15px, 600, 1.2, normal |
| Card title | `type.heading.card` | Control, 13px, 600, 1.2, normal |
| Body copy | `type.body.sm` | Sans, 13px, 400, 1.5, normal |
| Supporting text | `type.support` | Control, 11px, 400, 1.55, normal |
| Control label | `type.label.md` | Control, 13px, 400, 1.2, normal |
| Selected control label | `type.label.selected` | Control, 13px, 600, 1.2, normal |

Use every property from the selected role so localization, platform font
fallbacks, density, and the runtime font-size preference remain synchronized:

```css
.title {
  font-family: var(--openbitfun-type-heading-card-font-family);
  font-size: var(--openbitfun-type-heading-card-font-size);
  font-weight: var(--openbitfun-type-heading-card-font-weight);
  line-height: var(--openbitfun-type-heading-card-line-height);
  letter-spacing: var(--openbitfun-type-heading-card-letter-spacing);
}
```

Foundation variables such as `--openbitfun-font-size-sm` remain available for renderer
adapters and non-text geometry. Public text components should use `--openbitfun-type-*`
roles.

When an existing composition intentionally overrides only line height or
tracking, use `type.modifier.leading.*` or `type.modifier.tracking.*` on top of
its established role. `type.overline.*` owns extra-small uppercase annotations;
these modifiers keep product styles semantic without changing their resolved
metrics during migration. `type.modifier.leading.support` provides the compact
1.45 supporting-text rhythm used when an 11px role must align to a 16px line.
