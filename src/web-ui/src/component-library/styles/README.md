[中文](README.zh-CN.md) | **English**

# Design Tokens

This directory defines BitFun component design tokens to unify colors, typography, spacing, shadows, motion, and layering.

## Files

- `tokens.scss`: token and composite token definitions

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
  background: $card-bg-default;
  border: 1px solid var(--border-base);
  box-shadow: var(--shadow-sm);
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
