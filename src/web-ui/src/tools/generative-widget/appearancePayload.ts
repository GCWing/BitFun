import { widgetAppearanceAdapter } from '@/infrastructure/appearance/adapters/WidgetAppearanceAdapter';
import { WIDGET_APPEARANCE_VARIABLE_NAMES } from '@/infrastructure/appearance/adapters/widgetAppearanceVariables';
import { getBuiltinAppearanceCssToken } from '@/infrastructure/appearance/builtins/catalog';

export type WidgetAppearancePayload = {
  id: string;
  mode: string;
  vars: Record<string, string>;
};

const FALLBACK_VAR = {
  bgPrimary: '--bf-appearance-token-color-bg-primary',
  bgElevated: '--bf-appearance-token-color-bg-elevated',
  bgTertiary: '--bf-appearance-token-color-bg-tertiary',
  bgScene: '--bf-appearance-token-color-bg-scene',
  textPrimary: '--bf-appearance-token-color-text-primary',
  textSecondary: '--bf-appearance-token-color-text-secondary',
  textMuted: '--bf-appearance-token-color-text-muted',
  textDisabled: '--bf-appearance-token-color-text-disabled',
  accent50: '--bf-appearance-token-color-accent-50',
  accent100: '--bf-appearance-token-color-accent-100',
  accent400: '--bf-appearance-token-color-accent-400',
  accent500: '--bf-appearance-token-color-accent-500',
  accent500Rgb: '--bf-appearance-token-color-accent-500-rgb',
  accent600: '--bf-appearance-token-color-accent-600',
  bgSecondary: '--bf-appearance-token-color-bg-secondary',
  success: '--bf-appearance-token-color-success',
  successBg: '--bf-appearance-token-color-success-bg',
  warning: '--bf-appearance-token-color-warning',
  warningBg: '--bf-appearance-token-color-warning-bg',
  error: '--bf-appearance-token-color-error',
  errorBg: '--bf-appearance-token-color-error-bg',
  errorBorder: '--bf-appearance-token-color-error-border',
  staticWhite: '--bf-appearance-token-color-static-white',
  staticBlack: '--bf-appearance-token-color-static-black',
  overlayWhite04: '--bf-appearance-token-color-overlay-white-04',
  overlayBlack08: '--bf-appearance-token-color-overlay-black-08',
  overlayBlack30: '--bf-appearance-token-color-overlay-black-30',
  borderSubtle: '--bf-appearance-token-border-subtle',
  borderBase: '--bf-appearance-token-border-base',
  borderMedium: '--bf-appearance-token-border-medium',
  elementBgSubtle: '--bf-appearance-token-element-bg-subtle',
  elementBgBase: '--bf-appearance-token-element-bg-base',
  elementBgMedium: '--bf-appearance-token-element-bg-medium',
  elementBgSoft: '--bf-appearance-token-element-bg-soft',
  elementBgHover: '--bf-appearance-token-element-bg-hover',
  motionBase: '--bf-appearance-token-motion-base',
  shadowXs: '--bf-appearance-token-shadow-xs',
  shadowSm: '--bf-appearance-token-shadow-sm',
  sizeRadiusSm: '--bf-appearance-token-size-radius-sm',
  sizeRadiusBase: '--bf-appearance-token-size-radius-base',
  sizeRadiusMd: '--bf-appearance-token-size-radius-md',
  sizeRadiusLg: '--bf-appearance-token-size-radius-lg',
  sizeRadiusXl: '--bf-appearance-token-size-radius-xl',
  sizeRadius2xl: '--bf-appearance-token-size-radius-2xl',
  sizeRadiusFull: '--bf-appearance-token-size-radius-full',
  sizeGap1: '--bf-appearance-token-size-gap-1',
  sizeGap2: '--bf-appearance-token-size-gap-2',
  sizeGap3: '--bf-appearance-token-size-gap-3',
  sizeGap4: '--bf-appearance-token-size-gap-4',
  sizeGap5: '--bf-appearance-token-size-gap-5',
  sizeGap6: '--bf-appearance-token-size-gap-6',
  sizeGap8: '--bf-appearance-token-size-gap-8',
  sizeGap10: '--bf-appearance-token-size-gap-10',
  sizeGap12: '--bf-appearance-token-size-gap-12',
  sizeGap16: '--bf-appearance-token-size-gap-16',
} as const;

type WidgetAppearanceFallbackVarName = typeof FALLBACK_VAR[keyof typeof FALLBACK_VAR];
const widgetVar = (name: string): string => `var(${name})`;
const builtinToken = (name: WidgetAppearanceFallbackVarName): string => getBuiltinAppearanceCssToken(name);

const WIDGET_APPEARANCE_FALLBACK_COLOR = {
  textPrimary: builtinToken(FALLBACK_VAR.textPrimary),
  textSecondary: builtinToken(FALLBACK_VAR.textSecondary),
  textMuted: builtinToken(FALLBACK_VAR.textMuted),
  accent500: builtinToken(FALLBACK_VAR.accent500),
  accent600: builtinToken(FALLBACK_VAR.accent600),
  bgSecondary: builtinToken(FALLBACK_VAR.bgSecondary),
  success: builtinToken(FALLBACK_VAR.success),
  warning: builtinToken(FALLBACK_VAR.warning),
  error: builtinToken(FALLBACK_VAR.error),
  staticWhite: builtinToken(FALLBACK_VAR.staticWhite),
  staticBlack: builtinToken(FALLBACK_VAR.staticBlack),
  borderSubtle: builtinToken(FALLBACK_VAR.borderSubtle),
  borderBase: builtinToken(FALLBACK_VAR.borderBase),
  borderMedium: builtinToken(FALLBACK_VAR.borderMedium),
  elementBgSubtle: builtinToken(FALLBACK_VAR.elementBgSubtle),
  elementBgBase: builtinToken(FALLBACK_VAR.elementBgBase),
  elementBgMedium: builtinToken(FALLBACK_VAR.elementBgMedium),
  shadowBase: builtinToken(FALLBACK_VAR.overlayBlack30),
} as const;

// Keep this fallback map small and self-contained. It is the last-resort iframe
// contract for static widget rendering before the host Appearance payload arrives.
export const WIDGET_APPEARANCE_FALLBACK_VARS = {
  [FALLBACK_VAR.bgPrimary]: 'transparent',
  [FALLBACK_VAR.bgElevated]: WIDGET_APPEARANCE_FALLBACK_COLOR.bgSecondary,
  [FALLBACK_VAR.bgTertiary]: WIDGET_APPEARANCE_FALLBACK_COLOR.bgSecondary,
  [FALLBACK_VAR.bgScene]: WIDGET_APPEARANCE_FALLBACK_COLOR.bgSecondary,
  [FALLBACK_VAR.textPrimary]: WIDGET_APPEARANCE_FALLBACK_COLOR.textPrimary,
  [FALLBACK_VAR.textSecondary]: WIDGET_APPEARANCE_FALLBACK_COLOR.textSecondary,
  [FALLBACK_VAR.textMuted]: WIDGET_APPEARANCE_FALLBACK_COLOR.textMuted,
  [FALLBACK_VAR.textDisabled]: WIDGET_APPEARANCE_FALLBACK_COLOR.textMuted,
  [FALLBACK_VAR.accent50]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent500)} 10%, transparent)`,
  [FALLBACK_VAR.accent100]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent500)} 16%, transparent)`,
  [FALLBACK_VAR.accent400]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent500)} 78%, ${widgetVar(FALLBACK_VAR.staticWhite)})`,
  [FALLBACK_VAR.accent500]: WIDGET_APPEARANCE_FALLBACK_COLOR.accent500,
  [FALLBACK_VAR.accent500Rgb]: '96 165 250',
  [FALLBACK_VAR.accent600]: WIDGET_APPEARANCE_FALLBACK_COLOR.accent600,
  [FALLBACK_VAR.bgSecondary]: WIDGET_APPEARANCE_FALLBACK_COLOR.bgSecondary,
  [FALLBACK_VAR.success]: WIDGET_APPEARANCE_FALLBACK_COLOR.success,
  [FALLBACK_VAR.successBg]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.success)} 16%, transparent)`,
  [FALLBACK_VAR.warning]: WIDGET_APPEARANCE_FALLBACK_COLOR.warning,
  [FALLBACK_VAR.warningBg]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.warning)} 16%, transparent)`,
  [FALLBACK_VAR.error]: WIDGET_APPEARANCE_FALLBACK_COLOR.error,
  [FALLBACK_VAR.errorBg]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.error)} 16%, transparent)`,
  [FALLBACK_VAR.errorBorder]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.error)} 34%, transparent)`,
  [FALLBACK_VAR.staticWhite]: WIDGET_APPEARANCE_FALLBACK_COLOR.staticWhite,
  [FALLBACK_VAR.staticBlack]: WIDGET_APPEARANCE_FALLBACK_COLOR.staticBlack,
  [FALLBACK_VAR.overlayWhite04]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticWhite)} 4%, transparent)`,
  [FALLBACK_VAR.overlayBlack08]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 8%, transparent)`,
  [FALLBACK_VAR.overlayBlack30]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 30%, transparent)`,
  [FALLBACK_VAR.borderSubtle]: WIDGET_APPEARANCE_FALLBACK_COLOR.borderSubtle,
  [FALLBACK_VAR.borderBase]: WIDGET_APPEARANCE_FALLBACK_COLOR.borderBase,
  [FALLBACK_VAR.borderMedium]: WIDGET_APPEARANCE_FALLBACK_COLOR.borderMedium,
  [FALLBACK_VAR.elementBgSubtle]: WIDGET_APPEARANCE_FALLBACK_COLOR.elementBgSubtle,
  [FALLBACK_VAR.elementBgBase]: WIDGET_APPEARANCE_FALLBACK_COLOR.elementBgBase,
  [FALLBACK_VAR.elementBgMedium]: WIDGET_APPEARANCE_FALLBACK_COLOR.elementBgMedium,
  [FALLBACK_VAR.elementBgSoft]: WIDGET_APPEARANCE_FALLBACK_COLOR.elementBgBase,
  [FALLBACK_VAR.elementBgHover]: WIDGET_APPEARANCE_FALLBACK_COLOR.elementBgMedium,
  [FALLBACK_VAR.motionBase]: '0.2s',
  [FALLBACK_VAR.shadowXs]: `0 1px 2px ${WIDGET_APPEARANCE_FALLBACK_COLOR.shadowBase}`,
  [FALLBACK_VAR.shadowSm]: `0 2px 4px ${WIDGET_APPEARANCE_FALLBACK_COLOR.shadowBase}`,
  [FALLBACK_VAR.sizeRadiusSm]: '6px',
  [FALLBACK_VAR.sizeRadiusBase]: '8px',
  [FALLBACK_VAR.sizeRadiusMd]: widgetVar(FALLBACK_VAR.sizeRadiusBase),
  [FALLBACK_VAR.sizeRadiusLg]: '12px',
  [FALLBACK_VAR.sizeRadiusXl]: '16px',
  [FALLBACK_VAR.sizeRadius2xl]: '20px',
  [FALLBACK_VAR.sizeRadiusFull]: '9999px',
  [FALLBACK_VAR.sizeGap1]: '4px',
  [FALLBACK_VAR.sizeGap2]: '8px',
  [FALLBACK_VAR.sizeGap3]: '12px',
  [FALLBACK_VAR.sizeGap4]: '16px',
  [FALLBACK_VAR.sizeGap5]: '20px',
  [FALLBACK_VAR.sizeGap6]: '24px',
  [FALLBACK_VAR.sizeGap8]: '32px',
  [FALLBACK_VAR.sizeGap10]: '40px',
  [FALLBACK_VAR.sizeGap12]: '48px',
  [FALLBACK_VAR.sizeGap16]: '64px',
} as const satisfies Record<WidgetAppearanceFallbackVarName, string>;

export function createWidgetAppearanceFallbackCss(): string {
  return Object.entries(WIDGET_APPEARANCE_FALLBACK_VARS)
    .map(([name, value]) => `      ${name}: ${value};`)
    .join('\n');
}

const WIDGET_APPEARANCE_STATIC_SHELL_VARS = {
  '--bf-appearance-token-color-bg-quaternary': widgetVar(FALLBACK_VAR.bgSecondary),
  '--bf-appearance-token-color-bg-workbench': widgetVar(FALLBACK_VAR.bgPrimary),
  '--bf-appearance-token-color-bg-tooltip': widgetVar(FALLBACK_VAR.bgElevated),
  '--bf-appearance-token-color-overlay-white-08': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticWhite)} 8%, transparent)`,
  '--bf-appearance-token-color-overlay-white-12': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticWhite)} 12%, transparent)`,
  '--bf-appearance-token-color-overlay-white-15': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticWhite)} 15%, transparent)`,
  '--bf-appearance-token-color-overlay-white-20': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticWhite)} 20%, transparent)`,
  '--bf-appearance-token-color-overlay-white-60': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticWhite)} 60%, transparent)`,
  '--bf-appearance-token-color-overlay-black-12': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 12%, transparent)`,
  '--bf-appearance-token-color-overlay-black-15': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 15%, transparent)`,
  '--bf-appearance-token-color-overlay-black-20': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 20%, transparent)`,
  '--bf-appearance-token-color-overlay-black-40': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 40%, transparent)`,
  '--bf-appearance-token-color-overlay-black-50': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 50%, transparent)`,
  '--bf-appearance-token-color-overlay-black-80': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 80%, transparent)`,
  '--bf-appearance-token-color-accent-200': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent500)} 24%, transparent)`,
  '--bf-appearance-token-color-accent-300': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent500)} 34%, transparent)`,
  '--bf-appearance-token-color-accent-700': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent600)} 86%, ${widgetVar(FALLBACK_VAR.staticBlack)})`,
  '--bf-appearance-token-color-accent-800': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent600)} 92%, ${widgetVar(FALLBACK_VAR.staticBlack)})`,
  '--bf-appearance-token-color-success-border': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.success)} 34%, transparent)`,
  '--bf-appearance-token-color-warning-border': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.warning)} 34%, transparent)`,
  '--bf-appearance-token-color-info': widgetVar(FALLBACK_VAR.textMuted),
  '--bf-appearance-token-color-info-bg': widgetVar(FALLBACK_VAR.elementBgSubtle),
  '--bf-appearance-token-color-info-border': widgetVar(FALLBACK_VAR.borderMedium),
  '--bf-appearance-token-border-strong': widgetVar(FALLBACK_VAR.borderMedium),
  '--bf-appearance-token-border-prominent': widgetVar(FALLBACK_VAR.borderMedium),
  '--bf-appearance-token-element-bg-strong': widgetVar(FALLBACK_VAR.elementBgMedium),
  '--bf-appearance-token-scrollbar-thumb': widgetVar(FALLBACK_VAR.elementBgMedium),
  '--bf-appearance-token-scrollbar-thumb-hover': widgetVar('--bf-appearance-token-element-bg-strong'),
  '--bf-appearance-token-glass-bg-base': widgetVar(FALLBACK_VAR.elementBgBase),
  '--bf-appearance-token-glass-bg-hover': widgetVar(FALLBACK_VAR.elementBgMedium),
  '--bf-appearance-token-glass-bg-active': widgetVar('--bf-appearance-token-element-bg-strong'),
  '--bf-appearance-token-glass-border-base': widgetVar(FALLBACK_VAR.borderBase),
  '--bf-appearance-token-glass-border-hover': widgetVar(FALLBACK_VAR.borderMedium),
  '--bf-appearance-token-shadow-base': `0 4px 8px ${widgetVar(FALLBACK_VAR.overlayBlack30)}`,
  '--bf-appearance-token-shadow-lg': `0 8px 16px ${widgetVar(FALLBACK_VAR.overlayBlack30)}`,
  '--bf-appearance-token-shadow-xl': `0 12px 24px ${widgetVar(FALLBACK_VAR.overlayBlack30)}`,
  '--bf-appearance-token-font-size-xxs': '10px',
  '--bf-appearance-token-font-size-2xs': '11px',
  '--bf-appearance-token-font-size-xl': '16px',
  '--bf-appearance-token-opacity-disabled': '0.6',
  '--bf-appearance-token-opacity-hover': '0.8',
  '--bf-appearance-token-opacity-focus': '0.9',
  '--bf-appearance-token-motion-slow': '0.6s',
  '--bf-appearance-token-tool-card-font-mono': widgetVar('--bf-appearance-token-font-family-mono'),
  '--bf-appearance-token-btn-primary-bg': widgetVar(FALLBACK_VAR.accent100),
  '--bf-appearance-token-btn-primary-color': widgetVar(FALLBACK_VAR.accent600),
  '--bf-appearance-token-btn-primary-border': 'transparent',
  '--bf-appearance-token-btn-primary-shadow': 'none',
  '--bf-appearance-token-btn-primary-hover-bg': widgetVar(FALLBACK_VAR.accent400),
  '--bf-appearance-token-btn-primary-hover-color': widgetVar(FALLBACK_VAR.textPrimary),
  '--bf-appearance-token-btn-primary-hover-border': 'transparent',
  '--bf-appearance-token-btn-primary-hover-shadow': 'none',
  '--bf-appearance-token-btn-primary-hover-transform': 'none',
  '--bf-appearance-token-btn-primary-active-bg': widgetVar(FALLBACK_VAR.accent100),
  '--bf-appearance-token-btn-primary-active-color': widgetVar(FALLBACK_VAR.textPrimary),
  '--bf-appearance-token-btn-primary-active-border': 'transparent',
  '--bf-appearance-token-btn-primary-active-shadow': 'none',
  '--bf-appearance-token-btn-primary-active-transform': 'none',
  '--bf-appearance-token-btn-ghost-color': widgetVar(FALLBACK_VAR.textMuted),
  '--bf-appearance-token-btn-ghost-hover-bg': widgetVar(FALLBACK_VAR.elementBgSubtle),
  '--bf-appearance-token-btn-ghost-hover-color': widgetVar(FALLBACK_VAR.textPrimary),
  '--bf-appearance-token-btn-ghost-hover-border': 'transparent',
  '--bf-appearance-token-tool-card-header-pad-y': '0.44rem',
  '--bf-appearance-token-tool-card-header-pad-right': '0.625rem',
  '--bf-appearance-token-tool-card-header-icon-slot': '34px',
  '--bf-appearance-token-tool-card-expanded-pad-x': '0.625rem',
  '--bf-appearance-token-tool-card-action-font-size': widgetVar('--bf-appearance-token-font-size-sm'),
  '--bf-appearance-token-tool-card-action-line-height': '1.45',
} as const;

export const WIDGET_APPEARANCE_STATIC_SHELL_VAR_NAMES = Object.keys(WIDGET_APPEARANCE_STATIC_SHELL_VARS);

export function createWidgetAppearanceStaticShellCss(): string {
  return Object.entries(WIDGET_APPEARANCE_STATIC_SHELL_VARS)
    .map(([name, value]) => `      ${name}: ${value};`)
    .join('\n');
}

export const WIDGET_APPEARANCE_VAR_NAMES = WIDGET_APPEARANCE_VARIABLE_NAMES;

export function readWidgetAppearancePayload(): WidgetAppearancePayload | null {
  const settings = widgetAppearanceAdapter.getSettings();
  const vars: Record<string, string> = {};
  for (const name of WIDGET_APPEARANCE_VAR_NAMES) {
    const value = settings.vars[name]
      ?? WIDGET_APPEARANCE_FALLBACK_VARS[name as keyof typeof WIDGET_APPEARANCE_FALLBACK_VARS];
    if (value) vars[name] = value;
  }

  return {
    id: settings.id,
    mode: settings.mode,
    vars,
  };
}
