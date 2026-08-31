[中文](README.zh-CN.md) | **English**

# Design Tokens

This directory defines BitFun component design tokens to unify colors, typography, spacing, shadows, motion, and layering.

## Files

- `tokens.scss`: token and composite token definitions
- `_overlay-surfaces.scss`: semantic chrome for temporary overlay surfaces

## Overlay surfaces

Choose the contract from interaction semantics, not size or component name:

- `floating-surface`: non-modal transient cards, including anchored menus, pickers, popovers, notifications, toasts, and status hints. It adds optional backdrop blur to the canonical popup-card chrome.
- `dialog-surface`: modal or focus-trapping surfaces, normally centered. It uses an opaque elevated background so underlying application content never shows through, while keeping the same canonical border, 12px radius, and shadow as the device overview.
- Transparent native windows may pass `$backdrop-blur: false` when blur causes whole-window recomposition. Border, radius, background, and shadow are still read from the same private chrome owner.

These are the only two public popup-card visual contracts. Product styles may define dimensions, placement, content layout, and state, but must not redefine their outer border, radius, background, or shadow. Content cards, tooltips, and fullscreen surfaces are not popup cards and keep their own semantic contracts.

## Usage

### Import in components

```scss
@import '../../styles/tokens.scss';

.my-component {
  background: $color-bg-primary;
  color: $color-text-primary;
  border: 1px solid $border-base;
  padding: $size-gap-4;
  border-radius: $size-radius-base;
  box-shadow: $shadow-base;
  transition: all $motion-base $easing-standard;
}
```

### Composite tokens

```scss
@import '../../styles/tokens.scss';

.card {
  background: var(--bf-color-surface-subtle);
  border: 1px solid var(--bf-color-border-default);
  box-shadow: var(--bf-shadow-sm);
}
```

### Export as CSS variables (optional)

```scss
@import '../../styles/tokens.scss';

:root {
  @include apply-design-tokens;
}
```

## Naming

- Base: `$color-*`, `$size-*`, `$font-*`, `$shadow-*`, `$motion-*`, `$easing-*`, `$z-*`
- Composite: `$panel-*`, `$card-*`, `$input-*`, `$modal-*`, `$nav-*`, `$button-*`

## Best Practices

- Prefer base tokens
- Use composite tokens for common patterns
- Avoid hard-coded values and keep names semantic

## Extending

1. Add new variables in `tokens.scss`
2. Follow the naming rules
3. Add composite tokens when needed
4. Update the `DesignTokens` preview
