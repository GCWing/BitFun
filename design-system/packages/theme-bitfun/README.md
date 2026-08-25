# @bitfun/theme-bitfun

Replaceable BitFun theme values for the framework-neutral `@bitfun/ui` package.

```ts
import "@bitfun/theme-bitfun/default.css";
import { themeModes, themeTokenCatalog, themes } from "@bitfun/theme-bitfun";
```

`default.css` synchronously imports the system token contract and all built-in theme variants. Theme selection is scoped to a `data-bf-design-system-root` element and uses independent `data-color-scheme` and `data-contrast` axes.

`themeTokenCatalog` exposes the complete public semantic theme contract for visual authoring: colors, elevation shadows, surface filters, and state opacity. UI components consume this semantic contract and never depend on reference colors directly.

## Foundational color scales

Named numeric scales live below the semantic theme layer. Step numbers increase from light to dark, for example `ref.color.neutral.50`, `ref.color.blue.500`, and `ref.color.red.700`. Semantic tokens map those stable palette values to roles such as `color.content.primary` or `color.status.danger.content` independently in each theme mode.

Design tools can read the palette through the authoring-only export:

```ts
import {
  referenceColorCatalog,
  referenceColorScales,
} from "@bitfun/theme-bitfun/authoring";
```

The same data is available as `@bitfun/theme-bitfun/reference-colors.json`. Reference colors deliberately do not emit runtime CSS variables; application and component CSS must continue to use semantic theme variables.
