import { WIDGET_IFRAME_FALLBACK_COLOR } from '@/shared/theme/themeBoundaryFallbacks';

export type WidgetThemePayload = {
  id: string;
  type: string;
  vars: Record<string, string>;
};

const FALLBACK_VAR = {
  bgPrimary: '--color-bg-primary',
  bgElevated: '--color-bg-elevated',
  bgTertiary: '--color-bg-tertiary',
  bgScene: '--color-bg-scene',
  textPrimary: '--color-text-primary',
  textSecondary: '--color-text-secondary',
  textMuted: '--color-text-muted',
  textDisabled: '--color-text-disabled',
  accent50: '--color-accent-50',
  accent100: '--color-accent-100',
  accent400: '--color-accent-400',
  accent500: '--color-accent-500',
  accent500Rgb: '--color-accent-500-rgb',
  accent600: '--color-accent-600',
  bgSecondary: '--color-bg-secondary',
  success: '--color-success',
  successBg: '--color-success-bg',
  warning: '--color-warning',
  warningBg: '--color-warning-bg',
  error: '--color-error',
  errorBg: '--color-error-bg',
  errorBorder: '--color-error-border',
  staticWhite: '--color-static-white',
  staticBlack: '--color-static-black',
  overlayWhite04: '--color-overlay-white-04',
  overlayBlack08: '--color-overlay-black-08',
  overlayBlack30: '--color-overlay-black-30',
  borderSubtle: '--border-subtle',
  borderBase: '--border-base',
  borderMedium: '--border-medium',
  elementBgSubtle: '--element-bg-subtle',
  elementBgBase: '--element-bg-base',
  elementBgMedium: '--element-bg-medium',
  elementBgSoft: '--element-bg-soft',
  elementBgElevated: '--element-bg-elevated',
  elementBgHover: '--element-bg-hover',
  motionBase: '--motion-base',
  shadowXs: '--shadow-xs',
  shadowSm: '--shadow-sm',
  sizeRadiusSm: '--size-radius-sm',
  sizeRadiusBase: '--size-radius-base',
  sizeRadiusMd: '--size-radius-md',
  sizeRadiusLg: '--size-radius-lg',
  sizeRadiusXl: '--size-radius-xl',
  sizeRadius2xl: '--size-radius-2xl',
  sizeRadiusFull: '--size-radius-full',
  sizeGap1: '--size-gap-1',
  sizeGap2: '--size-gap-2',
  sizeGap3: '--size-gap-3',
  sizeGap4: '--size-gap-4',
  sizeGap5: '--size-gap-5',
  sizeGap6: '--size-gap-6',
  sizeGap8: '--size-gap-8',
  sizeGap10: '--size-gap-10',
  sizeGap12: '--size-gap-12',
  sizeGap16: '--size-gap-16',
} as const;

type WidgetThemeFallbackVarName = typeof FALLBACK_VAR[keyof typeof FALLBACK_VAR];
const widgetVar = (name: string): string => `var(${name})`;

// Keep this fallback map small and self-contained. It is the last-resort iframe
// contract for static widget rendering before the host theme payload arrives.
export const WIDGET_THEME_FALLBACK_VARS = {
  [FALLBACK_VAR.bgPrimary]: 'transparent',
  [FALLBACK_VAR.bgElevated]: WIDGET_IFRAME_FALLBACK_COLOR.bgSecondary,
  [FALLBACK_VAR.bgTertiary]: WIDGET_IFRAME_FALLBACK_COLOR.bgSecondary,
  [FALLBACK_VAR.bgScene]: WIDGET_IFRAME_FALLBACK_COLOR.bgSecondary,
  [FALLBACK_VAR.textPrimary]: WIDGET_IFRAME_FALLBACK_COLOR.textPrimary,
  [FALLBACK_VAR.textSecondary]: WIDGET_IFRAME_FALLBACK_COLOR.textSecondary,
  [FALLBACK_VAR.textMuted]: WIDGET_IFRAME_FALLBACK_COLOR.textMuted,
  [FALLBACK_VAR.textDisabled]: WIDGET_IFRAME_FALLBACK_COLOR.textMuted,
  [FALLBACK_VAR.accent50]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent500)} 10%, transparent)`,
  [FALLBACK_VAR.accent100]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent500)} 16%, transparent)`,
  [FALLBACK_VAR.accent400]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent500)} 78%, ${widgetVar(FALLBACK_VAR.staticWhite)})`,
  [FALLBACK_VAR.accent500]: WIDGET_IFRAME_FALLBACK_COLOR.accent500,
  [FALLBACK_VAR.accent500Rgb]: '96 165 250',
  [FALLBACK_VAR.accent600]: WIDGET_IFRAME_FALLBACK_COLOR.accent600,
  [FALLBACK_VAR.bgSecondary]: WIDGET_IFRAME_FALLBACK_COLOR.bgSecondary,
  [FALLBACK_VAR.success]: WIDGET_IFRAME_FALLBACK_COLOR.success,
  [FALLBACK_VAR.successBg]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.success)} 16%, transparent)`,
  [FALLBACK_VAR.warning]: WIDGET_IFRAME_FALLBACK_COLOR.warning,
  [FALLBACK_VAR.warningBg]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.warning)} 16%, transparent)`,
  [FALLBACK_VAR.error]: WIDGET_IFRAME_FALLBACK_COLOR.error,
  [FALLBACK_VAR.errorBg]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.error)} 16%, transparent)`,
  [FALLBACK_VAR.errorBorder]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.error)} 34%, transparent)`,
  [FALLBACK_VAR.staticWhite]: WIDGET_IFRAME_FALLBACK_COLOR.staticWhite,
  [FALLBACK_VAR.staticBlack]: WIDGET_IFRAME_FALLBACK_COLOR.staticBlack,
  [FALLBACK_VAR.overlayWhite04]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticWhite)} 4%, transparent)`,
  [FALLBACK_VAR.overlayBlack08]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 8%, transparent)`,
  [FALLBACK_VAR.overlayBlack30]: `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 30%, transparent)`,
  [FALLBACK_VAR.borderSubtle]: WIDGET_IFRAME_FALLBACK_COLOR.borderSubtle,
  [FALLBACK_VAR.borderBase]: WIDGET_IFRAME_FALLBACK_COLOR.borderBase,
  [FALLBACK_VAR.borderMedium]: WIDGET_IFRAME_FALLBACK_COLOR.borderMedium,
  [FALLBACK_VAR.elementBgSubtle]: WIDGET_IFRAME_FALLBACK_COLOR.elementBgSubtle,
  [FALLBACK_VAR.elementBgBase]: WIDGET_IFRAME_FALLBACK_COLOR.elementBgBase,
  [FALLBACK_VAR.elementBgMedium]: WIDGET_IFRAME_FALLBACK_COLOR.elementBgMedium,
  [FALLBACK_VAR.elementBgSoft]: WIDGET_IFRAME_FALLBACK_COLOR.elementBgBase,
  [FALLBACK_VAR.elementBgElevated]: WIDGET_IFRAME_FALLBACK_COLOR.elementBgBase,
  [FALLBACK_VAR.elementBgHover]: WIDGET_IFRAME_FALLBACK_COLOR.elementBgMedium,
  [FALLBACK_VAR.motionBase]: '0.2s',
  [FALLBACK_VAR.shadowXs]: `0 1px 2px ${WIDGET_IFRAME_FALLBACK_COLOR.shadowBase}`,
  [FALLBACK_VAR.shadowSm]: `0 2px 4px ${WIDGET_IFRAME_FALLBACK_COLOR.shadowBase}`,
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
} as const satisfies Record<WidgetThemeFallbackVarName, string>;

export function createWidgetThemeFallbackCss(): string {
  return Object.entries(WIDGET_THEME_FALLBACK_VARS)
    .map(([name, value]) => `      ${name}: ${value};`)
    .join('\n');
}

const WIDGET_THEME_STATIC_SHELL_VARS = {
  '--color-bg-quaternary': widgetVar(FALLBACK_VAR.bgSecondary),
  '--color-bg-workbench': widgetVar(FALLBACK_VAR.bgPrimary),
  '--color-bg-tooltip': widgetVar(FALLBACK_VAR.bgElevated),
  '--color-overlay-white-08': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticWhite)} 8%, transparent)`,
  '--color-overlay-white-12': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticWhite)} 12%, transparent)`,
  '--color-overlay-white-15': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticWhite)} 15%, transparent)`,
  '--color-overlay-white-20': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticWhite)} 20%, transparent)`,
  '--color-overlay-white-60': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticWhite)} 60%, transparent)`,
  '--color-overlay-black-12': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 12%, transparent)`,
  '--color-overlay-black-15': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 15%, transparent)`,
  '--color-overlay-black-20': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 20%, transparent)`,
  '--color-overlay-black-40': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 40%, transparent)`,
  '--color-overlay-black-50': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 50%, transparent)`,
  '--color-overlay-black-80': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.staticBlack)} 80%, transparent)`,
  '--color-accent-200': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent500)} 24%, transparent)`,
  '--color-accent-300': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent500)} 34%, transparent)`,
  '--color-accent-700': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent600)} 86%, ${widgetVar(FALLBACK_VAR.staticBlack)})`,
  '--color-accent-800': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.accent600)} 92%, ${widgetVar(FALLBACK_VAR.staticBlack)})`,
  '--color-success-border': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.success)} 34%, transparent)`,
  '--color-warning-border': `color-mix(in srgb, ${widgetVar(FALLBACK_VAR.warning)} 34%, transparent)`,
  '--color-info': widgetVar(FALLBACK_VAR.textMuted),
  '--color-info-bg': widgetVar(FALLBACK_VAR.elementBgSubtle),
  '--color-info-border': widgetVar(FALLBACK_VAR.borderMedium),
  '--border-strong': widgetVar(FALLBACK_VAR.borderMedium),
  '--border-prominent': widgetVar(FALLBACK_VAR.borderMedium),
  '--element-bg-strong': widgetVar(FALLBACK_VAR.elementBgMedium),
  '--scrollbar-thumb': widgetVar(FALLBACK_VAR.elementBgMedium),
  '--scrollbar-thumb-hover': widgetVar(FALLBACK_VAR.elementBgElevated),
  '--color-scrollbar': widgetVar(FALLBACK_VAR.elementBgMedium),
  '--glass-base': widgetVar(FALLBACK_VAR.elementBgSubtle),
  '--glass-bg-base': widgetVar(FALLBACK_VAR.elementBgBase),
  '--glass-bg-hover': widgetVar(FALLBACK_VAR.elementBgMedium),
  '--glass-bg-active': widgetVar(FALLBACK_VAR.elementBgElevated),
  '--glass-border-base': widgetVar(FALLBACK_VAR.borderBase),
  '--glass-border-hover': widgetVar(FALLBACK_VAR.borderMedium),
  '--glass-border-focus': widgetVar(FALLBACK_VAR.accent400),
  '--glass-blur-sm': 'blur(4px)',
  '--glass-blur-base': 'blur(8px)',
  '--glass-shadow-sm': widgetVar(FALLBACK_VAR.shadowSm),
  '--glass-shadow-base': `0 4px 8px ${widgetVar(FALLBACK_VAR.overlayBlack30)}`,
  '--glass-shadow-lg': `0 8px 16px ${widgetVar(FALLBACK_VAR.overlayBlack30)}`,
  '--glass-shadow-xl': `0 12px 24px ${widgetVar(FALLBACK_VAR.overlayBlack30)}`,
  '--shadow-base': `0 4px 8px ${widgetVar(FALLBACK_VAR.overlayBlack30)}`,
  '--shadow-lg': `0 8px 16px ${widgetVar(FALLBACK_VAR.overlayBlack30)}`,
  '--shadow-xl': `0 12px 24px ${widgetVar(FALLBACK_VAR.overlayBlack30)}`,
  '--font-size-xxs': '10px',
  '--font-size-2xs': '11px',
  '--font-size-xl': '16px',
  '--opacity-disabled': '0.6',
  '--opacity-hover': '0.8',
  '--opacity-focus': '0.9',
  '--motion-slow': '0.6s',
  '--smooth-height-collapse-duration': '0.6s',
  '--tool-card-font-mono': widgetVar('--font-family-mono'),
  '--btn-primary-bg': widgetVar(FALLBACK_VAR.accent100),
  '--btn-primary-color': widgetVar(FALLBACK_VAR.accent600),
  '--btn-primary-border': 'transparent',
  '--btn-primary-shadow': 'none',
  '--btn-primary-hover-bg': widgetVar(FALLBACK_VAR.accent400),
  '--btn-primary-hover-color': widgetVar(FALLBACK_VAR.textPrimary),
  '--btn-primary-hover-border': 'transparent',
  '--btn-primary-hover-shadow': 'none',
  '--btn-primary-hover-transform': 'none',
  '--btn-primary-active-bg': widgetVar(FALLBACK_VAR.accent100),
  '--btn-primary-active-color': widgetVar(FALLBACK_VAR.textPrimary),
  '--btn-primary-active-border': 'transparent',
  '--btn-primary-active-shadow': 'none',
  '--btn-primary-active-transform': 'none',
  '--btn-ghost-color': widgetVar(FALLBACK_VAR.textMuted),
  '--btn-ghost-hover-bg': widgetVar(FALLBACK_VAR.elementBgSubtle),
  '--btn-ghost-hover-color': widgetVar(FALLBACK_VAR.textPrimary),
  '--btn-ghost-hover-border': 'transparent',
  '--tool-card-header-pad-y': '0.44rem',
  '--tool-card-header-pad-x': '0',
  '--tool-card-header-pad-right': '0.625rem',
  '--tool-card-header-icon-rail': '24px',
  '--tool-card-header-icon-slot': '34px',
  '--tool-card-icon-size': '16px',
  '--tool-card-expanded-pad-y': '0.5rem',
  '--tool-card-expanded-pad-x': '0.625rem',
  '--tool-card-action-font-size': widgetVar('--font-size-sm'),
  '--tool-card-action-line-height': '1.45',
  '--tool-card-action-font-weight': widgetVar('--font-weight-medium'),
} as const;

export const WIDGET_THEME_STATIC_SHELL_VAR_NAMES = Object.keys(WIDGET_THEME_STATIC_SHELL_VARS);

export function createWidgetThemeStaticShellCss(): string {
  return Object.entries(WIDGET_THEME_STATIC_SHELL_VARS)
    .map(([name, value]) => `      ${name}: ${value};`)
    .join('\n');
}

// Host -> generated-widget iframe theme contract. Keep groups explicit so
// isolated widgets receive stable tokens without scraping every root variable.
// Host chrome/layout internals stay private: widgets have their own stacking
// and layout context, so FlowChat, navigation, and z-index keys are not exported.
const WIDGET_THEME_VAR_GROUPS = {
  staticAndOverlay: [
    FALLBACK_VAR.bgPrimary,
    FALLBACK_VAR.staticWhite,
    FALLBACK_VAR.staticBlack,
  ],
  backgroundSurface: [
    FALLBACK_VAR.bgSecondary,
    FALLBACK_VAR.bgTertiary,
    FALLBACK_VAR.bgElevated,
    FALLBACK_VAR.bgScene,
  ],
  text: [
    FALLBACK_VAR.textPrimary,
    FALLBACK_VAR.textSecondary,
    FALLBACK_VAR.textMuted,
    FALLBACK_VAR.textDisabled,
  ],
  accent: [
    FALLBACK_VAR.accent500,
    FALLBACK_VAR.accent500Rgb,
    FALLBACK_VAR.accent600,
  ],
  semantic: [
    FALLBACK_VAR.success,
    FALLBACK_VAR.successBg,
    FALLBACK_VAR.warning,
    FALLBACK_VAR.warningBg,
    FALLBACK_VAR.error,
    FALLBACK_VAR.errorBg,
    '--color-info',
    '--color-info-bg',
    '--color-info-border',
  ],
  border: [
    FALLBACK_VAR.borderSubtle,
    FALLBACK_VAR.borderBase,
    FALLBACK_VAR.borderMedium,
  ],
  elementGlassShadow: [
    FALLBACK_VAR.elementBgSubtle,
    FALLBACK_VAR.elementBgSoft,
    FALLBACK_VAR.elementBgBase,
    FALLBACK_VAR.elementBgMedium,
    FALLBACK_VAR.elementBgElevated,
    FALLBACK_VAR.elementBgHover,
    FALLBACK_VAR.shadowXs,
    FALLBACK_VAR.shadowSm,
  ],
  shapeSpacingTypography: [
    FALLBACK_VAR.sizeRadiusSm,
    FALLBACK_VAR.sizeRadiusBase,
    FALLBACK_VAR.sizeRadiusLg,
    FALLBACK_VAR.sizeRadiusXl,
    FALLBACK_VAR.sizeRadius2xl,
    FALLBACK_VAR.sizeRadiusFull,
    FALLBACK_VAR.sizeGap1,
    FALLBACK_VAR.sizeGap2,
    FALLBACK_VAR.sizeGap3,
    FALLBACK_VAR.sizeGap4,
    FALLBACK_VAR.sizeGap5,
    FALLBACK_VAR.sizeGap6,
    FALLBACK_VAR.sizeGap8,
    FALLBACK_VAR.sizeGap10,
    FALLBACK_VAR.sizeGap12,
    FALLBACK_VAR.sizeGap16,
    '--font-size-xs',
    '--font-size-sm',
    '--font-size-base',
    '--font-size-lg',
    '--font-size-2xl',
    '--font-weight-medium',
    '--font-weight-semibold',
  ],
  motionAndFonts: [
    '--motion-fast',
    FALLBACK_VAR.motionBase,
    '--easing-standard',
    '--font-family-sans',
    '--font-family-mono',
  ],
  buttons: [
    '--btn-primary-bg',
    '--btn-primary-color',
    '--btn-primary-border',
    '--btn-primary-shadow',
    '--btn-primary-hover-bg',
    '--btn-primary-hover-color',
    '--btn-primary-hover-border',
    '--btn-primary-hover-shadow',
    '--btn-primary-hover-transform',
    '--btn-primary-active-bg',
    '--btn-primary-active-color',
    '--btn-primary-active-border',
    '--btn-primary-active-shadow',
    '--btn-primary-active-transform',
    '--btn-ghost-color',
    '--btn-ghost-hover-bg',
    '--btn-ghost-hover-color',
    '--btn-ghost-hover-border',
  ],
} as const;

const WIDGET_THEME_VAR_NAMES = Object.values(WIDGET_THEME_VAR_GROUPS).flat();

export function readWidgetThemePayload(): WidgetThemePayload | null {
  if (typeof window === 'undefined' || typeof document === 'undefined') {
    return null;
  }

  const root = document.documentElement;
  const styles = window.getComputedStyle(root);
  const vars: Record<string, string> = {};

  for (const name of WIDGET_THEME_VAR_NAMES) {
    const value = styles.getPropertyValue(name).trim();
    if (value) {
      vars[name] = value;
    } else if (name in WIDGET_THEME_FALLBACK_VARS) {
      vars[name] = WIDGET_THEME_FALLBACK_VARS[name as keyof typeof WIDGET_THEME_FALLBACK_VARS];
    }
  }

  return {
    id: root.getAttribute('data-theme') || 'unknown',
    type: root.getAttribute('data-theme-type') || 'dark',
    vars,
  };
}
