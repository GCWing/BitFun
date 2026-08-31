import type React from 'react';
import type { CanvasColor, CanvasCommonStyleProps, CanvasTone } from './types';

export const usageColorSequence = [
  'gray',
  'purple',
  'green',
  'yellow',
  'pink',
  'blue',
  'orange',
] as const satisfies readonly CanvasColor[];

export const categoryPaletteLight: Record<CanvasColor, string> = {
  gray: 'var(--bf-color-content-muted)',
  purple: 'var(--bf-color-accent-default)',
  green: 'var(--bf-color-status-success-content)',
  yellow: 'var(--bf-color-status-warning-content)',
  cyan: 'var(--bf-color-status-info-content)',
  pink: 'var(--bitfun-canvas-danger)',
  blue: 'var(--bf-color-accent-default)',
  orange: 'var(--bf-color-status-warning-content)',
};

export const categoryPaletteDark = categoryPaletteLight;

export const canvasTokensLight = {
  bg: 'var(--bf-color-surface-canvas)',
  panel: 'var(--bf-color-surface-panel)',
  elevated: 'var(--bf-color-surface-raised)',
  chrome: 'var(--bf-color-surface-chrome)',
  text: 'var(--bf-color-content-primary)',
  textSecondary: 'var(--bf-color-content-secondary)',
  textMuted: 'var(--bf-color-content-muted)',
  border: 'var(--bf-color-border-subtle)',
  accent: 'var(--bf-color-accent-default)',
  success: 'var(--bf-color-status-success-content)',
  warning: 'var(--bf-color-status-warning-content)',
  danger: 'var(--bf-color-status-danger-content)',
  info: 'var(--bitfun-canvas-info)',
};

export const canvasTokens = canvasTokensLight;
export const canvasPaletteLight = categoryPaletteLight;
export const canvasPaletteDark = categoryPaletteDark;

export function mergeStyle(
  base: React.CSSProperties,
  override?: React.CSSProperties,
): React.CSSProperties {
  return { ...base, ...(override || {}) };
}

export function categoryColor(color: CanvasColor | undefined, index = 0): string {
  const resolved = color || usageColorSequence[index % usageColorSequence.length] || 'gray';
  return categoryPaletteLight[resolved] || categoryPaletteLight.gray;
}

export function spacingValue(value: unknown): string | unknown {
  return typeof value === 'number' ? `${value}px` : value;
}

export function spacingStyle(value: CanvasCommonStyleProps['padding'], property: string): React.CSSProperties {
  if (value === undefined || value === null) return {};
  if (typeof value === 'object') {
    const result: Record<string, unknown> = {};
    if (value.x !== undefined) {
      result[`${property}Left`] = spacingValue(value.x);
      result[`${property}Right`] = spacingValue(value.x);
    }
    if (value.y !== undefined) {
      result[`${property}Top`] = spacingValue(value.y);
      result[`${property}Bottom`] = spacingValue(value.y);
    }
    for (const key of ['top', 'right', 'bottom', 'left'] as const) {
      if (value[key] !== undefined) {
        result[property + key[0].toUpperCase() + key.slice(1)] = spacingValue(value[key]);
      }
    }
    return result as React.CSSProperties;
  }
  return { [property]: spacingValue(value) } as React.CSSProperties;
}

export function commonStyle(
  props: CanvasCommonStyleProps = {},
  style: React.CSSProperties = {},
): React.CSSProperties {
  const result: Record<string, unknown> = {
    ...spacingStyle(props.padding, 'padding'),
    ...spacingStyle(props.margin, 'margin'),
    ...(props.background !== undefined ? { background: props.background } : {}),
    ...(props.border !== undefined ? { border: props.border } : {}),
    ...(props.borderTop !== undefined ? { borderTop: props.borderTop } : {}),
    ...(props.borderRight !== undefined ? { borderRight: props.borderRight } : {}),
    ...(props.borderBottom !== undefined ? { borderBottom: props.borderBottom } : {}),
    ...(props.borderLeft !== undefined ? { borderLeft: props.borderLeft } : {}),
    ...(props.borderRadius !== undefined ? { borderRadius: spacingValue(props.borderRadius) } : {}),
    ...(props.width !== undefined ? { width: spacingValue(props.width) } : {}),
    ...(props.height !== undefined ? { height: spacingValue(props.height) } : {}),
    ...(props.flex !== undefined ? { flex: props.flex } : {}),
    ...(props.display !== undefined ? { display: props.display } : {}),
    ...(props.color !== undefined ? { color: props.color } : {}),
    ...(props.opacity !== undefined ? { opacity: props.opacity } : {}),
    ...(props.minWidth !== undefined ? { minWidth: spacingValue(props.minWidth) } : {}),
    ...(props.maxWidth !== undefined ? { maxWidth: spacingValue(props.maxWidth) } : {}),
    ...(props.minHeight !== undefined ? { minHeight: spacingValue(props.minHeight) } : {}),
    ...(props.maxHeight !== undefined ? { maxHeight: spacingValue(props.maxHeight) } : {}),
    ...style,
  };
  return result as React.CSSProperties;
}

export function flexAlign(value: unknown): React.CSSProperties['alignItems'] {
  return value === 'start' ? 'flex-start' : value === 'end' ? 'flex-end' : (value as React.CSSProperties['alignItems']) || 'center';
}

export function flexJustify(value: unknown): React.CSSProperties['justifyContent'] {
  return value === 'start'
    ? 'flex-start'
    : value === 'end'
      ? 'flex-end'
      : (value as React.CSSProperties['justifyContent']) || 'flex-start';
}

export function sizeValue(size: unknown): string {
  if (typeof size === 'number') return `${size}px`;
  return size === 'small' || size === 'sm'
    ? 'var(--bf-type-body-xs-font-size)'
    : size === 'lg'
      ? 'var(--bf-type-body-lg-font-size)'
      : size === 'body' || size === 'md' || !size
        ? 'var(--bf-type-body-sm-font-size)'
        : String(size);
}

export function weightValue(weight: unknown): number | string {
  return weight === 'medium'
    ? 'var(--bf-font-weight-medium)'
    : weight === 'semibold'
      ? 'var(--bf-font-weight-semibold)'
      : weight === 'bold'
        ? 'var(--bf-font-weight-bold)'
        : (weight as number | string | undefined) || 'var(--bf-font-weight-regular)';
}

export function toneColor(tone: CanvasTone | undefined): string {
  if (tone === 'success') return 'var(--bitfun-canvas-success)';
  if (tone === 'warning') return 'var(--bitfun-canvas-warning)';
  if (tone === 'danger' || tone === 'error') return 'var(--bitfun-canvas-danger)';
  if (tone === 'info') return 'var(--bitfun-canvas-info)';
  if (
    tone === 'secondary' ||
    tone === 'tertiary' ||
    tone === 'quaternary' ||
    tone === 'muted' ||
    tone === 'neutral'
  ) {
    return 'var(--bf-color-content-muted)';
  }
  return 'var(--bf-color-content-primary)';
}
