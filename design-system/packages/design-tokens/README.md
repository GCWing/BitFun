# @bitfun/design-tokens

Framework-neutral token contract and system scales for BitFun UI packages.

This package intentionally contains no concrete brand palette. Install a theme package such as `@bitfun/theme-bitfun` alongside it.

```ts
import {
  cssVariables,
  tokenCatalog,
  tokenModes,
  tokens,
} from "@bitfun/design-tokens";
import "@bitfun/design-tokens/tokens.css";
```

Only semantic and system token names are public API. Density modes reuse the same names and override values through a scoped `data-density` attribute.

`tokenCatalog` is the authoring contract for visual tools. Each entry exposes its type, CSS variable, category, description, and resolved value in every density mode. Applications normally consume `tokens`; editors consume the catalog instead of maintaining a second token list.
