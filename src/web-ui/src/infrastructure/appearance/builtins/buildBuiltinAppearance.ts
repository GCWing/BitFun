import type { AppearancePalette } from './AppearancePalette';
import type {
  AppearanceColorValue,
  AppearanceDurationValue,
  AppearanceFontFamilyValue,
  AppearanceLengthValue,
  AppearanceNumberValue,
  AppearancePackage,
  AppearanceReference,
  AppearanceStyle,
} from '../types';
import {
  WIDGET_APPEARANCE_VARIABLE_NAMES,
  type WidgetAppearanceVariableName,
} from '../adapters/widgetAppearanceVariables';

const ref = (path: string): AppearanceReference => ({ kind: 'ref', path });
const colorRef = (id: string): AppearanceReference => ref(`globals.colors.${id}`);
const lengthRef = (id: string): AppearanceReference => ref(`globals.lengths.${id}`);
const numberRef = (id: string): AppearanceReference => ref(`globals.numbers.${id}`);
const durationRef = (id: string): AppearanceReference => ref(`globals.durations.${id}`);
const easingRef = (id: string): AppearanceReference => ref(`globals.easings.${id}`);
const fontRef = (id: string): AppearanceReference => ref(`globals.fontFamilies.${id}`);

function color(value: string): AppearanceColorValue {
  const trimmed = value.trim();
  if (trimmed === 'transparent') return { kind: 'transparent' };
  if (/^#[0-9a-f]{3,8}$/i.test(trimmed)) return { kind: 'hex', value: trimmed };
  const rgb = /^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)(?:\s*,\s*([\d.]+))?\s*\)$/i.exec(trimmed);
  if (rgb) {
    return {
      kind: 'rgb',
      r: Number(rgb[1]),
      g: Number(rgb[2]),
      b: Number(rgb[3]),
      ...(rgb[4] === undefined ? {} : { a: Number(rgb[4]) }),
    };
  }
  const hsl = /^hsla?\(\s*([\d.]+)\s*,\s*([\d.]+)%\s*,\s*([\d.]+)%(?:\s*,\s*([\d.]+))?\s*\)$/i.exec(trimmed);
  if (hsl) {
    return {
      kind: 'hsl',
      h: Number(hsl[1]),
      s: Number(hsl[2]),
      l: Number(hsl[3]),
      ...(hsl[4] === undefined ? {} : { a: Number(hsl[4]) }),
    };
  }
  throw new Error(`Appearance palette color is invalid: ${value}`);
}

function length(value: string): AppearanceLengthValue {
  const trimmed = value.trim();
  if (trimmed === '0') return { kind: 'zero' };
  const match = /^(-?[\d.]+)(px|rem|%)$/.exec(trimmed);
  if (!match) throw new Error(`Appearance palette length is invalid: ${value}`);
  const unit = match[2] === '%' ? 'percent' : match[2];
  return { kind: unit, value: Number(match[1]) } as AppearanceLengthValue;
}

function duration(value: string): AppearanceDurationValue {
  const trimmed = value.trim();
  const match = /^([\d.]+)(ms|s)$/.exec(trimmed);
  if (!match) throw new Error(`Appearance palette duration is invalid: ${value}`);
  return { kind: 'ms', value: Number(match[1]) * (match[2] === 's' ? 1000 : 1) };
}

function number(value: number): AppearanceNumberValue {
  return { kind: 'number', value };
}

function fontFamily(value: string): AppearanceFontFamilyValue {
  const families = value.split(',').map(item => item.trim().replace(/^['"]|['"]$/g, '')).filter(Boolean);
  return { kind: 'fontFamily', families: families.slice(0, 8) };
}

function transition(property: 'all' | 'background-color' | 'border-color' | 'box-shadow' | 'color' | 'opacity' | 'transform' = 'all'): NonNullable<AppearanceStyle['transition']> {
  return [{ property, duration: durationRef('base'), easing: easingRef('standard') }];
}

function colorToRgbChannels(value: string): string | null {
  const trimmed = value.trim();
  const hex = /^#([0-9a-f]{6})$/i.exec(trimmed);
  if (hex) {
    const numeric = Number.parseInt(hex[1], 16);
    return `${(numeric >> 16) & 255} ${(numeric >> 8) & 255} ${numeric & 255}`;
  }
  const rgb = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/i.exec(trimmed);
  return rgb ? `${rgb[1]} ${rgb[2]} ${rgb[3]}` : null;
}

function createCssTokens(palette: AppearancePalette): Record<string, string> {
  const { colors, effects, motion, typography } = palette;
  const purple = colors.purple ?? {
    100: colors.element.subtle,
    200: colors.element.soft,
    500: colors.accent[500],
    600: colors.accent[600],
  };
  const tokens: Record<string, string> = {
    '--bf-appearance-token-color-bg-primary': colors.background.primary,
    '--bf-appearance-token-color-bg-secondary': colors.background.secondary,
    '--bf-appearance-token-color-bg-tertiary': colors.background.tertiary,
    '--bf-appearance-token-color-bg-elevated': colors.background.elevated,
    '--bf-appearance-token-color-bg-workbench': colors.background.workbench,
    '--bf-appearance-token-color-bg-scene': colors.background.scene,
    '--bf-appearance-token-color-static-white': '#ffffff',
    '--bf-appearance-token-color-static-black': '#000000',
    '--bf-appearance-token-color-static-white-rgb': '255, 255, 255',
    '--bf-appearance-token-color-static-black-rgb': '0, 0, 0',
    '--bf-appearance-token-color-accent-50': colors.accent[50],
    '--bf-appearance-token-color-accent-100': colors.accent[100],
    '--bf-appearance-token-color-accent-200': colors.accent[200],
    '--bf-appearance-token-color-accent-300': colors.accent[300],
    '--bf-appearance-token-color-accent-400': colors.accent[400],
    '--bf-appearance-token-color-accent-500': colors.accent[500],
    '--bf-appearance-token-color-accent-600': colors.accent[600],
    '--bf-appearance-token-color-accent-700': colors.accent[700],
    '--bf-appearance-token-color-accent-500-rgb': colorToRgbChannels(colors.accent[500])
      ?? colorToRgbChannels(colors.accent[600])
      ?? '96 165 250',
    '--bf-appearance-token-color-purple-100': purple[100],
    '--bf-appearance-token-color-purple-200': purple[200],
    '--bf-appearance-token-color-purple-500': purple[500],
    '--bf-appearance-token-color-purple-600': purple[600],
    '--bf-appearance-token-color-overlay-slate-rgb': '15, 23, 42',
    '--bf-appearance-token-color-text-primary': colors.text.primary,
    '--bf-appearance-token-color-text-secondary': colors.text.secondary,
    '--bf-appearance-token-color-text-muted': colors.text.muted,
    '--bf-appearance-token-color-text-disabled': colors.text.disabled,
    '--bf-appearance-token-color-success': colors.semantic.success,
    '--bf-appearance-token-color-success-bg': colors.semantic.successBg,
    '--bf-appearance-token-color-success-border': colors.semantic.successBorder,
    '--bf-appearance-token-color-warning': colors.semantic.warning,
    '--bf-appearance-token-color-warning-bg': colors.semantic.warningBg,
    '--bf-appearance-token-color-warning-border': colors.semantic.warningBorder,
    '--bf-appearance-token-color-error': colors.semantic.error,
    '--bf-appearance-token-color-error-bg': colors.semantic.errorBg,
    '--bf-appearance-token-color-error-border': colors.semantic.errorBorder,
    '--bf-appearance-token-color-info': colors.semantic.info,
    '--bf-appearance-token-color-info-bg': colors.semantic.infoBg,
    '--bf-appearance-token-color-info-border': colors.semantic.infoBorder,
    '--bf-appearance-token-border-subtle': colors.border.subtle,
    '--bf-appearance-token-border-base': colors.border.base,
    '--bf-appearance-token-border-medium': colors.border.medium,
    '--bf-appearance-token-border-strong': colors.border.strong,
    '--bf-appearance-token-border-prominent': colors.border.prominent,
    '--bf-appearance-token-border-accent-soft': `color-mix(in srgb, ${colors.accent[600]} 30%, transparent)`,
    '--bf-appearance-token-border-accent-subtle': `color-mix(in srgb, ${colors.accent[600]} 15%, transparent)`,
    '--bf-appearance-token-border-accent': `color-mix(in srgb, ${colors.accent[600]} 40%, transparent)`,
    '--bf-appearance-token-border-accent-strong': `color-mix(in srgb, ${colors.accent[600]} 60%, transparent)`,
    '--bf-appearance-token-scene-viewport-border-width': palette.layout?.sceneViewportBorder === false ? '0' : '1px',
    '--bf-appearance-token-element-bg-subtle': colors.element.subtle,
    '--bf-appearance-token-element-bg-soft': colors.element.soft,
    '--bf-appearance-token-element-bg-base': colors.element.base,
    '--bf-appearance-token-element-bg-medium': colors.element.medium,
    '--bf-appearance-token-element-bg-strong': colors.element.strong,
    '--bf-appearance-token-element-bg-hover': colors.element.medium,
    '--bf-appearance-token-git-color-branch': colors.git.branch,
    '--bf-appearance-token-git-color-branch-bg': colors.git.branchBg,
    '--bf-appearance-token-git-color-branch-bg-hover': colors.element.medium,
    '--bf-appearance-token-git-color-changes': colors.git.changes,
    '--bf-appearance-token-git-color-added': colors.git.added,
    '--bf-appearance-token-git-color-deleted': colors.git.deleted,
    '--bf-appearance-token-git-color-staged': colors.git.staged,
    '--bf-appearance-token-scrollbar-thumb': colors.scrollbar?.thumb ?? (palette.type === 'dark' ? 'rgba(255, 255, 255, 0.12)' : 'rgba(0, 0, 0, 0.15)'),
    '--bf-appearance-token-scrollbar-thumb-hover': colors.scrollbar?.thumbHover ?? (palette.type === 'dark' ? 'rgba(255, 255, 255, 0.24)' : 'rgba(0, 0, 0, 0.3)'),
    '--bf-appearance-token-opacity-disabled': String(effects.opacity.disabled),
    '--bf-appearance-token-opacity-hover': String(effects.opacity.hover),
    '--bf-appearance-token-badge-padding-y': '2px',
    '--bf-appearance-token-button-bg-hover': `linear-gradient(135deg, color-mix(in srgb, ${colors.accent[600]} 15%, transparent) 0%, color-mix(in srgb, ${colors.accent[500]} 12%, transparent) 30%, color-mix(in srgb, ${colors.accent[600]} 8%, transparent) 60%, color-mix(in srgb, ${colors.accent[600]} 18%, transparent) 100%)`,
    '--bf-appearance-token-glow-shadow-blue': `0 12px 32px color-mix(in srgb, ${colors.accent[600]} 25%, transparent), 0 6px 16px color-mix(in srgb, ${colors.accent[500]} 18%, transparent), 0 3px 8px rgba(0, 0, 0, 0.12)`,
    '--bf-appearance-token-inner-glow-top': 'inset 0 1px 0 rgba(255, 255, 255, 0.08)',
    '--bf-appearance-token-inner-glow-top-hover': 'inset 0 1px 0 rgba(255, 255, 255, 0.24)',
    '--bf-appearance-token-glass-blue-subtle': `color-mix(in srgb, ${colors.accent[600]} 8%, transparent)`,
    '--bf-appearance-token-glass-red-subtle': `color-mix(in srgb, ${colors.semantic.error} 5%, transparent)`,
    '--bf-appearance-token-glass-red-base': colors.semantic.errorBg,
    '--bf-appearance-token-glass-red-hover': `color-mix(in srgb, ${colors.semantic.error} 15%, transparent)`,
    '--bf-appearance-token-glass-bg-base': `linear-gradient(135deg, rgba(255, 255, 255, 0.04) 0%, rgba(255, 255, 255, 0.04) 50%, rgba(255, 255, 255, 0.04) 100%)`,
    '--bf-appearance-token-glass-bg-hover': colors.element.medium,
    '--bf-appearance-token-glass-bg-active': colors.element.strong,
    '--bf-appearance-token-glass-border-base': colors.border.base,
    '--bf-appearance-token-glass-border-hover': colors.border.medium,
    '--bf-appearance-token-color-indigo-500': '#6366f1',
    '--bf-appearance-token-color-cyan-500': '#06b6d4',
    '--bf-appearance-token-domain-context-compression': purple[500],
    '--bf-appearance-token-domain-generative-ui': '#06b6d4',
    '--bf-appearance-token-domain-mini-app': purple[500],
    '--bf-appearance-token-domain-mermaid-diagram': colors.semantic.success,
    '--bf-appearance-token-domain-tool-search': colors.accent[600],
    '--bf-appearance-token-domain-tool-web-search': '#06b6d4',
    '--bf-appearance-token-domain-tool-git': colors.semantic.warning,
    '--bf-appearance-token-domain-tool-terminal': '#14b8a6',
    '--bf-appearance-token-domain-tool-mcp': purple[500],
    '--bf-appearance-token-domain-tool-assistant-action': purple[500],
    '--bf-appearance-token-domain-tool-review-summary': '#06b6d4',
    '--bf-appearance-token-domain-capability-docs': colors.semantic.success,
    '--bf-appearance-token-domain-capability-testing': colors.semantic.warning,
    '--bf-appearance-token-domain-capability-creative': purple[500],
    '--bf-appearance-token-domain-capability-ops': '#06b6d4',
    '--bf-appearance-token-domain-insights-positive': colors.semantic.success,
    '--bf-appearance-token-domain-insights-time': purple[500],
    '--bf-appearance-token-domain-insights-neutral': colors.semantic.warning,
    '--bf-appearance-token-domain-insights-issue': colors.semantic.error,
    '--bf-appearance-token-domain-progress-compacting': '#14b8a6',
    '--bf-appearance-token-domain-template-memories': purple[500],
    '--bf-appearance-token-domain-review-member-default': '#64748b',
    '--bf-appearance-token-domain-review-worker': colors.accent[600],
    '--bf-appearance-token-domain-review-judge': purple[500],
    '--bf-appearance-token-domain-teal-action': '#14b8a6',
    '--bf-appearance-token-domain-todo': '#14b8a6',
    '--bf-appearance-token-domain-git-lane-0': colors.accent[600],
    '--bf-appearance-token-domain-git-lane-1': colors.semantic.success,
    '--bf-appearance-token-domain-git-lane-2': colors.semantic.warning,
    '--bf-appearance-token-domain-git-lane-3': purple[500],
    '--bf-appearance-token-domain-git-lane-4': colors.semantic.error,
    '--bf-appearance-token-domain-git-lane-5': '#06b6d4',
    '--bf-appearance-token-domain-git-lane-6': '#14b8a6',
    '--bf-appearance-token-domain-git-lane-7': '#64748b',
    '--bf-appearance-token-domain-text-stroke-0': colors.semantic.warning,
    '--bf-appearance-token-domain-text-stroke-1': colors.semantic.error,
    '--bf-appearance-token-domain-text-stroke-2': colors.accent[600],
    '--bf-appearance-token-domain-text-stroke-3': '#06b6d4',
    '--bf-appearance-token-domain-text-stroke-4': purple[500],
    '--bf-appearance-token-domain-inspector-active-border': colors.accent[600],
    '--bf-appearance-token-domain-inspector-active-background': `color-mix(in srgb, ${colors.accent[600]} 15%, transparent)`,
    '--bf-appearance-token-domain-inspector-active-border-subtle': `color-mix(in srgb, ${colors.accent[600]} 40%, transparent)`,
    '--bf-appearance-token-domain-inspector-selected-border': colors.semantic.success,
    '--bf-appearance-token-domain-inspector-selected-background': `color-mix(in srgb, ${colors.semantic.success} 18%, transparent)`,
    '--bf-appearance-token-domain-inspector-browser-tooltip-background': 'rgba(10, 10, 10, 0.92)',
    '--bf-appearance-token-domain-inspector-main-tooltip-background': 'rgba(15, 23, 42, 0.95)',
    '--bf-appearance-token-domain-inspector-tooltip-text': '#e2e8f0',
    '--bf-appearance-token-domain-inspector-tooltip-shadow': 'rgba(0, 0, 0, 0.5)',
    '--bf-appearance-token-language-blue': '#3178c6',
    '--bf-appearance-token-language-cyan': '#00add8',
    '--bf-appearance-token-language-yellow': '#f7df1e',
    '--bf-appearance-token-language-orange': '#e38c00',
    '--bf-appearance-token-language-red': colors.semantic.error,
    '--bf-appearance-token-language-green': colors.semantic.success,
    '--bf-appearance-token-language-purple': purple[500],
    '--bf-appearance-token-language-slate': '#64748b',
    '--bf-appearance-token-prism-light-foreground': '#24292f',
    '--bf-appearance-token-prism-light-comment': '#6e7781',
    '--bf-appearance-token-prism-light-keyword': '#cf222e',
    '--bf-appearance-token-prism-light-string': '#0550ae',
    '--bf-appearance-token-prism-light-function': '#8250df',
    '--bf-appearance-token-prism-light-number': '#0550ae',
    '--bf-appearance-token-prism-light-tag': '#116329',
    '--bf-appearance-token-prism-light-punctuation': '#57606a',
    '--bf-appearance-token-prism-light-property': '#953800',
    '--bf-appearance-token-prism-dark-foreground': '#d4d4d4',
    '--bf-appearance-token-prism-dark-comment': '#6a9955',
    '--bf-appearance-token-prism-dark-keyword': '#c586c0',
    '--bf-appearance-token-prism-dark-string': '#ce9178',
    '--bf-appearance-token-prism-dark-function': '#dcdcaa',
    '--bf-appearance-token-prism-dark-number': '#b5cea8',
    '--bf-appearance-token-prism-dark-tag': '#569cd6',
    '--bf-appearance-token-prism-dark-punctuation': '#d4d4d4',
    '--bf-appearance-token-prism-dark-property': '#9cdcfe',
    '--bf-appearance-token-blur-medium': 'blur(12px) saturate(1.2)',
    '--bf-appearance-token-blur-subtle': effects.blur.subtle,
    '--bf-appearance-token-blur-base': effects.blur.base,
    '--bf-appearance-token-motion-lazy': '1s',
    '--bf-appearance-token-motion-instant': motion.duration.instant,
    '--bf-appearance-token-easing-accelerate': 'cubic-bezier(0.4, 0, 1, 1)',
    '--bf-appearance-token-easing-decelerate': motion.easing.decelerate,
    '--bf-appearance-token-easing-smooth': motion.easing.smooth,
    '--bf-appearance-token-font-weight-bold': String(typography.weight.semibold),
    '--bf-appearance-token-font-weight-normal': String(typography.weight.normal),
    '--bf-appearance-token-font-size-3xl': typography.size['3xl'],
    '--bf-appearance-token-line-height-tight': String(typography.lineHeight.tight),
    '--bf-appearance-token-line-height-base': String(typography.lineHeight.base),
    '--bf-appearance-token-line-height-relaxed': String(typography.lineHeight.relaxed),
    '--bf-appearance-token-flowchat-font-size-xxs': '10px',
    '--bf-appearance-token-flowchat-font-size-2xs': '11px',
    '--bf-appearance-token-flowchat-font-size-xs': typography.size.xs,
    '--bf-appearance-token-flowchat-font-size-sm': typography.size.sm,
    '--bf-appearance-token-flowchat-font-size-base': typography.size.base,
    '--bf-appearance-token-flowchat-font-size-lg': typography.size.lg,
    '--bf-appearance-token-flowchat-font-size-xl': typography.size.xl,
    '--bf-appearance-token-flowchat-font-size-2xl': typography.size['2xl'],
    '--bf-appearance-token-flowchat-font-size-3xl': typography.size['3xl'],
    '--bf-appearance-token-flowchat-text-line-height': '1.58',
    '--bf-appearance-token-flowchat-support-line-height': '1.45',
    '--bf-appearance-token-flowchat-compact-line-height': '1.32',
    '--bf-appearance-token-flowchat-flow-item-gap': '0.42rem',
    '--bf-appearance-token-flowchat-turn-gap': '0.46rem',
    '--bf-appearance-token-flowchat-inline-gap': '0.35rem',
    '--bf-appearance-token-flowchat-control-gap': '0.5rem',
    '--bf-appearance-token-flowchat-control-pad-y': '0.35rem',
    '--bf-appearance-token-flowchat-control-pad-x': '0.75rem',
    '--bf-appearance-token-flowchat-content-inline-pad': '3rem',
    '--bf-appearance-token-flowchat-content-inline-pad-mobile': '1.5rem',
    // Reading column max-width (single source of truth, converged to 900px
    // hardcode on 2026-08-08): referenced uniformly by VirtualItemRenderer
    // (message column) / ExploreRegion (explore collapse group) / ChatInput
    // (input box + file edit bar) / BtwSessionPanel to avoid 900px drift.
    '--bf-appearance-token-flowchat-content-max-width': '900px',
    '--bf-appearance-token-flowchat-card-gap': '0.42rem',
    '--bf-appearance-token-flowchat-card-radius': effects.radius.base,
    '--bf-appearance-token-flowchat-card-pad-y': '0.625rem',
    '--bf-appearance-token-flowchat-card-pad-x': '0.75rem',
    '--bf-appearance-token-flowchat-card-expanded-pad-y': '0.5rem',
    '--bf-appearance-token-flowchat-card-expanded-pad-x': '0.625rem',
    '--bf-appearance-token-flowchat-code-font-size': typography.size.sm,
    '--bf-appearance-token-flowchat-code-line-height': '1.52',
    '--bf-appearance-token-flowchat-code-block-pad-y': '0.55rem',
    '--bf-appearance-token-flowchat-code-block-pad-x': '0.75rem',
    '--bf-appearance-token-flowchat-link-color': palette.type === 'dark' ? '#60a5fa' : '#0969da',
    '--bf-appearance-token-flowchat-link-hover-color': palette.type === 'dark' ? '#93c5fd' : '#0550ae',
    '--bf-appearance-token-tool-card-font-mono': typography.font.mono,
    '--bf-appearance-token-tool-card-header-pad-y': '0.44rem',
    '--bf-appearance-token-tool-card-header-pad-right': '0.625rem',
    '--bf-appearance-token-tool-card-header-icon-slot': '34px',
    '--bf-appearance-token-tool-card-expanded-pad-x': '0.625rem',
    '--bf-appearance-token-tool-card-action-font-size': typography.size.sm,
    '--bf-appearance-token-tool-card-action-line-height': '1.45',
  };

  const overlays = {
    '--bf-appearance-token-color-overlay-white-04': 'rgba(255, 255, 255, 0.04)',
    '--bf-appearance-token-color-overlay-white-08': 'rgba(255, 255, 255, 0.08)',
    '--bf-appearance-token-color-overlay-white-12': 'rgba(255, 255, 255, 0.12)',
    '--bf-appearance-token-color-overlay-white-15': 'rgba(255, 255, 255, 0.15)',
    '--bf-appearance-token-color-overlay-white-20': 'rgba(255, 255, 255, 0.2)',
    '--bf-appearance-token-color-overlay-white-24': 'rgba(255, 255, 255, 0.24)',
    '--bf-appearance-token-color-overlay-white-60': 'rgba(255, 255, 255, 0.6)',
    '--bf-appearance-token-color-overlay-white-70': 'rgba(255, 255, 255, 0.7)',
    '--bf-appearance-token-color-overlay-white-80': 'rgba(255, 255, 255, 0.8)',
    '--bf-appearance-token-color-overlay-white-90': 'rgba(255, 255, 255, 0.9)',
    '--bf-appearance-token-color-overlay-white-95': 'rgba(255, 255, 255, 0.95)',
    '--bf-appearance-token-color-overlay-black-08': 'rgba(0, 0, 0, 0.08)',
    '--bf-appearance-token-color-overlay-black-12': 'rgba(0, 0, 0, 0.12)',
    '--bf-appearance-token-color-overlay-black-15': 'rgba(0, 0, 0, 0.15)',
    '--bf-appearance-token-color-overlay-black-20': 'rgba(0, 0, 0, 0.2)',
    '--bf-appearance-token-color-overlay-black-30': 'rgba(0, 0, 0, 0.3)',
    '--bf-appearance-token-color-overlay-black-40': 'rgba(0, 0, 0, 0.4)',
    '--bf-appearance-token-color-overlay-black-50': 'rgba(0, 0, 0, 0.5)',
    '--bf-appearance-token-color-overlay-black-80': 'rgba(0, 0, 0, 0.8)',
    '--bf-appearance-token-color-overlay-slate-04': 'rgba(15, 23, 42, 0.04)',
    '--bf-appearance-token-color-overlay-slate-08': 'rgba(15, 23, 42, 0.08)',
    '--bf-appearance-token-color-overlay-slate-12': 'rgba(15, 23, 42, 0.12)',
  };
  Object.assign(tokens, overlays);
  Object.entries(purple).forEach(([key, value]) => { tokens[`--bf-appearance-token-color-purple-${key}`] = value; });
  Object.entries(effects.shadow).forEach(([key, value]) => { tokens[`--bf-appearance-token-shadow-${key}`] = value; });
  Object.entries(effects.blur).forEach(([key, value]) => { tokens[`--bf-appearance-token-blur-${key}`] = value; });
  Object.entries(effects.radius).forEach(([key, value]) => { tokens[`--bf-appearance-token-size-radius-${key}`] = value; });
  tokens['--bf-appearance-token-size-radius-md'] = effects.radius.base;
  Object.entries(effects.spacing).forEach(([key, value]) => { tokens[`--bf-appearance-token-size-gap-${key}`] = value; });
  Object.entries(motion.duration).forEach(([key, value]) => { tokens[`--bf-appearance-token-motion-${key}`] = value; });
  Object.entries(motion.easing).forEach(([key, value]) => { tokens[`--bf-appearance-token-easing-${key}`] = value; });
  tokens['--bf-appearance-token-font-family-sans'] = typography.font.sans;
  tokens['--bf-appearance-token-font-family-mono'] = typography.font.mono;
  Object.entries(typography.weight).forEach(([key, value]) => { tokens[`--bf-appearance-token-font-weight-${key}`] = String(value); });
  Object.entries(typography.size).forEach(([key, value]) => { tokens[`--bf-appearance-token-font-size-${key}`] = value; });
  Object.entries(typography.lineHeight).forEach(([key, value]) => { tokens[`--bf-appearance-token-line-height-${key}`] = String(value); });

  const button = palette.components?.button;
  tokens['--bf-appearance-token-btn-primary-bg'] = button?.primary.default.background ?? colors.accent[200];
  tokens['--bf-appearance-token-btn-primary-color'] = button?.primary.default.color ?? colors.accent[600];
  tokens['--bf-appearance-token-btn-primary-border'] = button?.primary.default.border ?? 'transparent';
  tokens['--bf-appearance-token-btn-primary-shadow'] = button?.primary.default.shadow ?? 'none';
  tokens['--bf-appearance-token-btn-primary-hover-bg'] = button?.primary.hover.background ?? colors.accent[300];
  tokens['--bf-appearance-token-btn-primary-hover-color'] = button?.primary.hover.color ?? colors.text.primary;
  tokens['--bf-appearance-token-btn-primary-hover-border'] = button?.primary.hover.border ?? 'transparent';
  tokens['--bf-appearance-token-btn-primary-hover-shadow'] = button?.primary.hover.shadow ?? 'none';
  tokens['--bf-appearance-token-btn-primary-hover-transform'] = button?.primary.hover.transform ?? 'none';
  tokens['--bf-appearance-token-btn-primary-active-bg'] = button?.primary.active.background ?? colors.accent[200];
  tokens['--bf-appearance-token-btn-primary-active-color'] = button?.primary.active.color ?? colors.text.primary;
  tokens['--bf-appearance-token-btn-primary-active-border'] = button?.primary.active.border ?? 'transparent';
  tokens['--bf-appearance-token-btn-primary-active-shadow'] = button?.primary.active.shadow ?? 'none';
  tokens['--bf-appearance-token-btn-primary-active-transform'] = button?.primary.active.transform ?? 'none';
  tokens['--bf-appearance-token-btn-ghost-color'] = button?.ghost.default.color ?? colors.text.muted;
  tokens['--bf-appearance-token-btn-ghost-hover-bg'] = button?.ghost.hover.background ?? colors.element.subtle;
  tokens['--bf-appearance-token-btn-ghost-hover-color'] = button?.ghost.hover.color ?? colors.text.primary;
  tokens['--bf-appearance-token-btn-ghost-hover-border'] = button?.ghost.hover.border ?? 'transparent';
  return tokens;
}

function createWidgetAppearanceVars(
  cssTokens: Readonly<Record<string, string>>,
): Record<WidgetAppearanceVariableName, string> {
  return Object.fromEntries(WIDGET_APPEARANCE_VARIABLE_NAMES.map(name => {
    const value = cssTokens[name];
    if (!value) throw new Error(`Builtin Appearance does not define Widget token ${name}`);
    return [name, value];
  })) as Record<WidgetAppearanceVariableName, string>;
}

export function buildBuiltinAppearance(palette: AppearancePalette): AppearancePackage {
  const cssTokens = createCssTokens(palette);
  const primaryButton = palette.components?.button?.primary;
  const ghostButton = palette.components?.button?.ghost;
  const purple = palette.colors.purple ?? {
    100: palette.colors.element.subtle,
    200: palette.colors.element.soft,
    500: palette.colors.accent[500],
    600: palette.colors.accent[600],
  };
  const colors = {
    'bg-primary': color(palette.colors.background.primary),
    'bg-elevated': color(palette.colors.background.elevated),
    'text-primary': color(palette.colors.text.primary),
    'text-secondary': color(palette.colors.text.secondary),
    'text-muted': color(palette.colors.text.muted),
    'accent': color(palette.colors.accent[500]),
    'accent-active': color(palette.colors.accent[600]),
    'border-subtle': color(palette.colors.border.subtle),
    'border-base': color(palette.colors.border.base),
    'border-strong': color(palette.colors.border.strong),
    'element-subtle': color(palette.colors.element.subtle),
    'element-soft': color(palette.colors.element.soft),
    'element-base': color(palette.colors.element.base),
    'element-medium': color(palette.colors.element.medium),
    'error': color(palette.colors.semantic.error),
    'error-bg': color(palette.colors.semantic.errorBg),
    'error-border': color(palette.colors.semantic.errorBorder),
    'success': color(palette.colors.semantic.success),
    'success-bg': color(palette.colors.semantic.successBg),
    'purple': color(purple[500]),
    'purple-bg': color(purple[100]),
    'button-primary-bg': color(primaryButton?.default.background ?? palette.colors.element.base),
    'button-primary-fg': color(primaryButton?.default.color ?? palette.colors.text.primary),
    'button-primary-border': color(primaryButton?.default.border ?? palette.colors.border.base),
    'button-primary-hover-bg': color(primaryButton?.hover.background ?? palette.colors.element.medium),
    'button-primary-hover-fg': color(primaryButton?.hover.color ?? palette.colors.text.primary),
    'button-primary-hover-border': color(primaryButton?.hover.border ?? palette.colors.border.medium),
    'button-primary-active-bg': color(primaryButton?.active.background ?? palette.colors.element.soft),
    'button-primary-active-fg': color(primaryButton?.active.color ?? palette.colors.text.primary),
    'button-primary-active-border': color(primaryButton?.active.border ?? palette.colors.border.strong),
    'button-ghost-hover-bg': color(ghostButton?.hover.background ?? palette.colors.element.soft),
    'button-ghost-hover-fg': color(ghostButton?.hover.color ?? palette.colors.text.primary),
  };
  const monaco = palette.monaco;
  const xtermAnsi = palette.type === 'dark' ? {
    black: '#000000', red: '#cd3131', green: '#0dbc79', yellow: '#e5e510', blue: '#2472c8',
    magenta: '#bc3fbc', cyan: '#11a8cd', white: '#e5e5e5', brightBlack: '#666666',
    brightRed: '#f14c4c', brightGreen: '#23d18b', brightYellow: '#f5f543', brightBlue: '#3b8eea',
    brightMagenta: '#d670d6', brightCyan: '#29b8db', brightWhite: '#e5e5e5',
  } : {
    black: '#000000', red: '#cd3131', green: '#107C10', yellow: '#949800', blue: '#0451a5',
    magenta: '#bc05bc', cyan: '#0598bc', white: '#555555', brightBlack: '#666666',
    brightRed: '#cd3131', brightGreen: '#14CE14', brightYellow: '#b5ba00', brightBlue: '#0451a5',
    brightMagenta: '#bc05bc', brightCyan: '#0598bc', brightWhite: '#a5a5a5',
  };
  const monacoColors: Record<string, string> = {
    'editor.background': monaco?.colors.background ?? palette.colors.background.scene,
    'editor.foreground': monaco?.colors.foreground ?? palette.colors.text.primary,
    'editorLineNumber.foreground': palette.colors.text.muted,
    'editorCursor.foreground': monaco?.colors.cursor ?? palette.colors.accent[500],
    'editor.selectionBackground': monaco?.colors.selection ?? palette.colors.accent[300],
    'editor.inactiveSelectionBackground': palette.colors.accent[200],
    'editor.selectionHighlightBackground': palette.colors.accent[200],
    'editor.selectionHighlightBorder': palette.colors.accent[400],
    'editor.wordHighlightBackground': palette.colors.accent[100],
    'editor.wordHighlightStrongBackground': palette.colors.accent[200],
    'editor.lineHighlightBackground': monaco?.colors.lineHighlight ?? palette.colors.background.secondary,
    focusBorder: '#00000000',
    contrastBorder: '#00000000',
    'diffEditor.insertedTextBorder': '#00000000',
    'diffEditor.removedTextBorder': '#00000000',
  };
  Object.entries(monaco?.colors ?? {}).forEach(([key, value]) => {
    if (!['background', 'foreground', 'lineHighlight', 'selection', 'cursor'].includes(key)) {
      monacoColors[key] = value;
    }
  });

  return {
    schema: 'bitfun.appearance',
    schemaVersion: 1,
    id: palette.id,
    name: palette.name,
    version: /^\d+\.\d+\.\d+/.test(palette.version ?? '') ? palette.version! : '1.0.0',
    mode: palette.type,
    requiredCapabilities: ['components.v1', 'renderers.v1'],
    globals: {
      colors,
      lengths: {
        'border-one': { kind: 'px', value: 1 },
        'radius-sm': length(palette.effects.radius.sm),
        'radius-base': length(palette.effects.radius.base),
        'radius-lg': length(palette.effects.radius.lg),
        'gap-1': length(palette.effects.spacing[1]),
        'gap-2': length(palette.effects.spacing[2]),
        'gap-3': length(palette.effects.spacing[3]),
        'gap-4': length(palette.effects.spacing[4]),
        'gap-6': length(palette.effects.spacing[6]),
        'font-xs': length(palette.typography.size.xs),
        'font-sm': length(palette.typography.size.sm),
        'font-base': length(palette.typography.size.base),
        'font-lg': length(palette.typography.size.lg),
      },
      numbers: {
        'opacity-disabled': number(palette.effects.opacity.disabled),
        'weight-medium': number(palette.typography.weight.medium),
        'weight-semibold': number(palette.typography.weight.semibold),
        'line-tight': number(palette.typography.lineHeight.tight),
        'line-base': number(palette.typography.lineHeight.base),
        'line-relaxed': number(palette.typography.lineHeight.relaxed),
      },
      durations: {
        fast: duration(palette.motion.duration.fast),
        base: duration(palette.motion.duration.base),
      },
      easings: {
        standard: { kind: 'easing', value: 'standard' },
      },
      fontFamilies: {
        sans: fontFamily(palette.typography.font.sans),
      },
    },
    materials: {
      control: {
        visualRole: 'control',
        style: {
          backgroundColor: colorRef('element-base'),
          color: colorRef('text-secondary'),
          borderRadius: lengthRef('radius-sm'),
          fontFamily: fontRef('sans'),
          transition: transition(),
        },
      },
      surface: {
        visualRole: 'card',
        style: {
          backgroundColor: colorRef('element-subtle'),
          color: colorRef('text-secondary'),
          borderRadius: lengthRef('radius-base'),
          transition: transition(),
        },
      },
    },
    components: {
      button: {
        parts: {
          root: {
            materials: ['control'],
            base: {
              borderStyle: 'solid',
              borderWidth: { kind: 'zero' },
              fontWeight: numberRef('weight-medium'),
              gap: lengthRef('gap-2'),
              cursor: 'pointer',
            },
            facets: {
              variant: {
                primary: {
                  backgroundColor: colorRef('button-primary-bg'),
                  color: colorRef('button-primary-fg'),
                  borderColor: colorRef('button-primary-border'),
                  borderWidth: lengthRef('border-one'),
                },
                accent: {
                  backgroundColor: colorRef('button-primary-bg'),
                  color: colorRef('button-primary-fg'),
                  borderColor: colorRef('button-primary-border'),
                  borderWidth: lengthRef('border-one'),
                },
                secondary: { backgroundColor: colorRef('element-base') },
                ghost: { backgroundColor: { kind: 'transparent' } },
                dashed: {
                  backgroundColor: { kind: 'transparent' },
                  borderColor: colorRef('border-base'),
                  borderStyle: 'dashed',
                  borderWidth: lengthRef('border-one'),
                },
                danger: { backgroundColor: colorRef('error-bg'), color: colorRef('error') },
                success: { backgroundColor: colorRef('success-bg'), color: colorRef('success') },
                ai: { backgroundColor: colorRef('purple-bg'), color: colorRef('purple') },
              },
              size: {
                small: { height: { kind: 'px', value: 32 }, paddingInline: lengthRef('gap-3'), fontSize: lengthRef('font-sm') },
                medium: { height: { kind: 'px', value: 40 }, paddingInline: lengthRef('gap-4'), fontSize: lengthRef('font-base') },
                large: { height: { kind: 'px', value: 48 }, paddingInline: lengthRef('gap-6'), fontSize: lengthRef('font-lg') },
              },
              iconOnly: {
                true: { paddingInline: { kind: 'zero' } },
              },
            },
            states: {
              hover: { backgroundColor: colorRef('element-medium'), color: colorRef('text-primary') },
              active: { backgroundColor: colorRef('element-soft') },
              focusVisible: { outlineColor: colorRef('accent'), outlineWidth: { kind: 'px', value: 2 }, outlineOffset: { kind: 'px', value: 1 } },
              disabled: { opacity: numberRef('opacity-disabled'), cursor: 'default' },
              loading: { cursor: 'wait' },
            },
            contexts: [
              { when: { facets: { variant: 'primary' }, states: ['hover'] }, style: { backgroundColor: colorRef('button-primary-hover-bg'), color: colorRef('button-primary-hover-fg'), borderColor: colorRef('button-primary-hover-border') } },
              { when: { facets: { variant: 'accent' }, states: ['hover'] }, style: { backgroundColor: colorRef('button-primary-hover-bg'), color: colorRef('button-primary-hover-fg'), borderColor: colorRef('button-primary-hover-border') } },
              { when: { facets: { variant: 'primary' }, states: ['active'] }, style: { backgroundColor: colorRef('button-primary-active-bg'), color: colorRef('button-primary-active-fg'), borderColor: colorRef('button-primary-active-border') } },
              { when: { facets: { variant: 'accent' }, states: ['active'] }, style: { backgroundColor: colorRef('button-primary-active-bg'), color: colorRef('button-primary-active-fg'), borderColor: colorRef('button-primary-active-border') } },
              { when: { facets: { variant: 'ghost' }, states: ['hover'] }, style: { backgroundColor: colorRef('button-ghost-hover-bg'), color: colorRef('button-ghost-hover-fg') } },
              { when: { facets: { size: 'small', iconOnly: 'true' } }, style: { width: { kind: 'px', value: 32 } } },
              { when: { facets: { size: 'medium', iconOnly: 'true' } }, style: { width: { kind: 'px', value: 40 } } },
              { when: { facets: { size: 'large', iconOnly: 'true' } }, style: { width: { kind: 'px', value: 48 } } },
            ],
          },
          loadingIcon: {
            base: { width: { kind: 'px', value: 16 }, height: { kind: 'px', value: 16 }, borderRadius: { kind: 'percent', value: 50 } },
          },
          loadingText: { base: { color: colorRef('text-primary') } },
        },
      },
      card: {
        parts: {
          root: {
            materials: ['surface'],
            base: { display: 'flex', flexDirection: 'column' },
            facets: {
              variant: {
                default: { backgroundColor: colorRef('element-subtle') },
                elevated: { backgroundColor: colorRef('element-soft') },
                subtle: { backgroundColor: { kind: 'transparent' } },
                accent: { backgroundColor: colorRef('element-soft') },
                purple: { backgroundColor: colorRef('purple-bg') },
              },
              padding: {
                none: { padding: { kind: 'zero' } },
                small: { padding: lengthRef('gap-2') },
                medium: { padding: lengthRef('gap-4') },
                large: { padding: lengthRef('gap-6') },
              },
              radius: {
                small: { borderRadius: lengthRef('radius-sm') },
                medium: { borderRadius: lengthRef('radius-base') },
                large: { borderRadius: lengthRef('radius-lg') },
              },
            },
            states: {
              fullWidth: { width: { kind: 'percent', value: 100 } },
            },
            contexts: [
              { when: { states: ['interactive', 'hover'] }, style: { backgroundColor: colorRef('element-soft'), cursor: 'pointer' } },
              { when: { states: ['interactive', 'active'] }, style: { backgroundColor: colorRef('element-base') } },
            ],
          },
          header: { base: { display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: lengthRef('gap-3') } },
          title: { base: { color: colorRef('text-primary'), fontSize: lengthRef('font-sm'), fontWeight: numberRef('weight-semibold'), lineHeight: numberRef('line-tight') } },
          subtitle: { base: { color: colorRef('text-muted'), fontSize: lengthRef('font-xs'), lineHeight: numberRef('line-base') } },
          body: { base: { color: colorRef('text-secondary'), fontSize: lengthRef('font-sm'), lineHeight: numberRef('line-relaxed') } },
          footer: { base: { display: 'flex', alignItems: 'center', gap: lengthRef('gap-2') }, facets: { align: { left: { justifyContent: 'flex-start' }, center: { justifyContent: 'center' }, right: { justifyContent: 'flex-end' }, between: { justifyContent: 'space-between' } } } },
        },
      },
      input: {
        parts: {
          root: { base: { display: 'flex', flexDirection: 'column', gap: { kind: 'px', value: 6 } }, states: { disabled: { opacity: numberRef('opacity-disabled'), cursor: 'default' } } },
          label: { base: { color: colorRef('text-secondary'), fontSize: lengthRef('font-sm'), fontWeight: numberRef('weight-medium') } },
          container: {
            base: { display: 'flex', alignItems: 'center', gap: lengthRef('gap-2'), paddingInline: lengthRef('gap-3'), borderRadius: { kind: 'px', value: 4 }, borderStyle: 'solid', borderWidth: lengthRef('border-one'), transition: transition() },
            facets: {
              variant: {
                default: { backgroundColor: { kind: 'transparent' }, borderColor: colorRef('border-base') },
                filled: { backgroundColor: colorRef('element-subtle'), borderColor: { kind: 'transparent' } },
                outlined: { backgroundColor: { kind: 'transparent' }, borderColor: colorRef('border-strong') },
              },
              size: {
                small: { height: { kind: 'px', value: 28 }, fontSize: { kind: 'px', value: 13 } },
                medium: { height: { kind: 'px', value: 36 }, fontSize: { kind: 'px', value: 14 } },
                large: { height: { kind: 'px', value: 44 }, fontSize: { kind: 'px', value: 15 } },
              },
            },
            states: {
              hover: { backgroundColor: colorRef('element-subtle'), borderColor: colorRef('border-strong') },
              focusWithin: { backgroundColor: { kind: 'transparent' }, borderColor: colorRef('accent') },
              error: { borderColor: colorRef('error-border') },
            },
          },
          control: { base: { color: colorRef('text-primary'), fontFamily: fontRef('sans') } },
          prefix: { base: { color: colorRef('text-muted') } },
          suffix: { base: { color: colorRef('text-muted') } },
          message: { base: { color: colorRef('error'), fontSize: lengthRef('font-xs') } },
        },
      },
      modal: {
        parts: {
          overlay: { base: { backgroundColor: { kind: 'rgb', r: 0, g: 0, b: 0, a: 0.55 }, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: { kind: 'px', value: 24 } } },
          dialog: {
            base: { backgroundColor: colorRef('bg-primary'), borderColor: colorRef('border-subtle'), borderStyle: 'solid', borderWidth: lengthRef('border-one'), borderRadius: { kind: 'px', value: 8 }, maxHeight: { kind: 'percent', value: 100 } },
            facets: {
              size: {
                small: { width: { kind: 'percent', value: 100 }, maxWidth: { kind: 'px', value: 420 } },
                medium: { width: { kind: 'percent', value: 100 }, maxWidth: { kind: 'px', value: 560 } },
                large: { width: { kind: 'percent', value: 100 }, maxWidth: { kind: 'px', value: 600 } },
                xlarge: { width: { kind: 'percent', value: 100 }, maxWidth: { kind: 'px', value: 720 } },
              },
            },
          },
          header: { base: { display: 'flex', alignItems: 'center', gap: { kind: 'px', value: 6 }, padding: { kind: 'px', value: 8 }, borderWidth: { kind: 'zero' }, borderBottomColor: colorRef('border-subtle'), borderBottomWidth: lengthRef('border-one'), borderStyle: 'solid' } },
          title: { base: { color: colorRef('text-secondary'), fontSize: { kind: 'px', value: 12 }, fontWeight: numberRef('weight-medium') } },
          close: { base: { backgroundColor: { kind: 'transparent' }, color: colorRef('text-muted'), borderRadius: { kind: 'px', value: 6 }, cursor: 'pointer' }, states: { hover: { backgroundColor: colorRef('element-subtle'), color: colorRef('text-primary') }, focusVisible: { outlineColor: colorRef('accent'), outlineWidth: { kind: 'px', value: 2 }, outlineOffset: { kind: 'px', value: 2 } } } },
          content: { base: { color: colorRef('text-secondary'), overflowY: 'auto', overflowX: 'hidden' }, states: { contentInset: { paddingInline: { kind: 'px', value: 28 } } } },
          resizeHandle: { states: { hover: { backgroundColor: colorRef('element-subtle') } } },
        },
      },
    },
    renderers: {
      'css-tokens': {
        version: 1,
        settings: {
          tokens: cssTokens,
          background: palette.colors.background.primary,
        },
      },
      monaco: {
        version: 1,
        settings: {
          id: `bf-appearance-${palette.id}`,
          base: monaco?.base ?? (palette.type === 'dark' ? 'vs-dark' : 'vs'),
          inherit: monaco?.inherit ?? true,
          rules: monaco?.rules ?? [],
          colors: monacoColors,
        },
      },
      xterm: {
        version: 1,
        settings: {
          surfaces: {
            terminal: {
              background: palette.colors.background.scene,
              foreground: palette.colors.text.primary,
              cursor: palette.colors.text.primary,
              cursorAccent: palette.colors.background.secondary,
              selectionBackground: palette.type === 'dark' ? 'rgba(255, 255, 255, 0.3)' : 'rgba(173, 214, 255, 0.45)',
              selectionInactiveBackground: palette.type === 'dark' ? 'rgba(255, 255, 255, 0.15)' : 'rgba(173, 214, 255, 0.25)',
              ...xtermAnsi,
            },
            output: {
              background: palette.colors.background.scene,
              foreground: palette.colors.text.primary,
              cursor: 'transparent',
              cursorAccent: 'transparent',
              selectionBackground: palette.type === 'dark' ? 'rgba(255, 255, 255, 0.3)' : 'rgba(173, 214, 255, 0.45)',
              selectionInactiveBackground: palette.type === 'dark' ? 'rgba(255, 255, 255, 0.15)' : 'rgba(173, 214, 255, 0.25)',
              ...xtermAnsi,
            },
          },
          fontWeight: palette.type === 'dark' ? 'normal' : '500',
          fontWeightBold: palette.type === 'dark' ? 'bold' : '700',
        },
      },
      mermaid: {
        version: 1,
        settings: {
          mode: palette.type,
          palette: {
            nodeFill: palette.colors.element.base,
            nodeFillHover: palette.colors.element.medium,
            nodeText: palette.colors.text.primary,
            nodeStroke: palette.colors.border.strong,
            nodeStrokeHover: palette.colors.accent[500],
            clusterFill: palette.colors.background.secondary,
            clusterText: palette.colors.text.secondary,
            clusterStroke: palette.colors.border.base,
            edgeStroke: palette.colors.text.muted,
            edgeLabelBackground: palette.colors.background.elevated,
            edgeLabelText: palette.colors.text.secondary,
            noteFill: palette.colors.semantic.warningBg,
            noteText: palette.colors.text.primary,
            noteStroke: palette.colors.semantic.warning,
            activationFill: palette.colors.element.medium,
            activationStroke: palette.colors.border.strong,
            success: palette.colors.semantic.success,
            warning: palette.colors.semantic.warning,
            error: palette.colors.semantic.error,
            errorBackground: palette.colors.semantic.errorBg,
            info: palette.colors.accent[500],
            highlight: palette.colors.accent[500],
            pieColors: [
              palette.colors.accent[500], palette.colors.semantic.success, palette.colors.semantic.warning,
              palette.colors.semantic.error, purple[500], palette.colors.accent[400],
              palette.colors.accent[600], palette.colors.text.muted,
            ],
          },
        },
      },
      'generative-widget': {
        version: 1,
        settings: {
          id: `builtin.${palette.id}`,
          mode: palette.type,
          vars: createWidgetAppearanceVars(cssTokens),
        },
      },
      'bitfun-canvas': {
        version: 1,
        settings: {
          id: `builtin.${palette.id}`,
          mode: palette.type,
          bg: palette.colors.background.primary,
          panel: palette.colors.background.secondary,
          fg: palette.colors.text.primary,
          muted: palette.colors.text.muted,
          border: palette.colors.border.base,
          accent: palette.colors.accent[500],
          success: palette.colors.semantic.success,
          warning: palette.colors.semantic.warning,
          danger: palette.colors.semantic.error,
          info: palette.colors.semantic.info,
        },
      },
    },
  };
}
