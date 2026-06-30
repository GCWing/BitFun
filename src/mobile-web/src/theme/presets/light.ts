import {
  alpha,
  colorRamp,
  commonMobileThemeVars,
  shadow,
  TRANSPARENT,
  type MobileThemeVars,
} from './shared';

const BLACK = '0, 0, 0';
const WHITE = '255, 255, 255';
const IOS_NEUTRAL = '120, 120, 128';
const BORDER = '60, 60, 67';
const ACCENT = '0, 122, 255';
const ACCENT_STRONG = '0, 102, 204';
const PURPLE = '175, 82, 222';
const PURPLE_STRONG = '149, 54, 204';
const PINK = '236, 72, 153';
const PINK_STRONG = '219, 39, 119';
const ORANGE = '255, 149, 0';
const ORANGE_STRONG = '204, 122, 0';
const SUCCESS = '52, 199, 89';
const ERROR = '255, 59, 48';

const BG_PRIMARY = '#f3f3f5';
const BG_SECONDARY = '#ffffff';
const BG_QUATERNARY = '#e5e5ea';
const TEXT_PRIMARY = '#1c1c1e';
const TEXT_SECONDARY = '#48484a';
const TEXT_MUTED = '#8e8e93';
const TEXT_DISABLED = '#aeaeb2';
const ACCENT_500 = '#007aff';
const ACCENT_600 = '#0066cc';
const PURPLE_500 = '#af52de';
const PURPLE_600 = '#9536cc';
const PINK_500 = '#ec4899';
const PINK_600 = '#db2777';
const ORANGE_500 = '#ff9500';
const ORANGE_600 = '#cc7a00';
const SUCCESS_500 = '#34c759';
const ERROR_500 = '#ff3b30';

export const lightTheme: MobileThemeVars = {
  '--color-bg-primary': BG_PRIMARY,
  '--color-bg-secondary': BG_SECONDARY,
  '--color-bg-tertiary': BG_PRIMARY,
  '--color-bg-quaternary': BG_QUATERNARY,
  '--color-bg-elevated': BG_SECONDARY,
  '--color-bg-workbench': BG_PRIMARY,
  '--color-bg-scene': BG_SECONDARY,
  '--color-bg-flowchat': BG_SECONDARY,
  '--color-bg-tooltip': alpha(WHITE, '0.98'),

  '--color-text-primary': TEXT_PRIMARY,
  '--color-text-secondary': TEXT_SECONDARY,
  '--color-text-muted': TEXT_MUTED,
  '--color-text-disabled': TEXT_DISABLED,

  '--element-bg-subtle': alpha(IOS_NEUTRAL, '0.04'),
  '--element-bg-soft': alpha(IOS_NEUTRAL, '0.08'),
  '--element-bg-base': alpha(IOS_NEUTRAL, '0.1'),
  '--element-bg-medium': alpha(IOS_NEUTRAL, '0.14'),
  '--element-bg-strong': alpha(IOS_NEUTRAL, '0.2'),
  '--element-bg-elevated': alpha(WHITE, '0.94'),

  ...colorRamp('--color-accent', ACCENT, ACCENT_500, ACCENT_600, ACCENT_STRONG, ['0.04', '0.08', '0.14', '0.22', '0.36']),
  ...colorRamp('--color-purple', PURPLE, PURPLE_500, PURPLE_600, PURPLE_STRONG, ['0.04', '0.08', '0.14', '0.22', '0.36']),
  ...colorRamp('--color-pink', PINK, PINK_500, PINK_600, PINK_STRONG, ['0.04', '0.08', '0.14', '0.22', '0.36']),
  ...colorRamp('--color-orange', ORANGE, ORANGE_500, ORANGE_600, ORANGE_STRONG, ['0.04', '0.08', '0.14', '0.22', '0.36']),

  '--color-success': SUCCESS_500,
  '--color-success-bg': alpha(SUCCESS, '0.08'),
  '--color-success-border': alpha(SUCCESS, '0.25'),
  '--color-warning': ORANGE_500,
  '--color-warning-bg': alpha(ORANGE, '0.08'),
  '--color-warning-border': alpha(ORANGE, '0.25'),
  '--color-error': ERROR_500,
  '--color-error-bg': alpha(ERROR, '0.08'),
  '--color-error-border': alpha(ERROR, '0.25'),
  '--color-info': ACCENT_500,
  '--color-info-bg': alpha(ACCENT, '0.08'),
  '--color-info-border': alpha(ACCENT, '0.25'),

  '--color-highlight': ORANGE_500,
  '--color-highlight-bg': alpha(ORANGE, '0.1'),
  '--color-overlay': alpha(BLACK, '0.3'),

  '--border-subtle': alpha(BORDER, '0.08'),
  '--border-base': alpha(BORDER, '0.14'),
  '--border-medium': alpha(BORDER, '0.22'),
  '--border-strong': alpha(BORDER, '0.3'),
  '--border-prominent': alpha(BORDER, '0.4'),

  '--glow-blue': `0 2px 8px ${alpha(ACCENT, '0.08')}`,
  '--glow-purple': `0 2px 8px ${alpha(PURPLE, '0.08')}`,
  '--glow-orange': `0 2px 8px ${alpha(ORANGE, '0.08')}`,
  '--glow-mixed': shadow(
    `0 2px 8px ${alpha(ACCENT, '0.06')}`,
    `0 2px 8px ${alpha(PURPLE, '0.04')}`,
  ),

  '--shadow-xs': `0 1px 2px ${alpha(BLACK, '0.04')}`,
  '--shadow-sm': shadow(
    `0 1px 3px ${alpha(BLACK, '0.06')}`,
    `0 1px 2px ${alpha(BLACK, '0.04')}`,
  ),
  '--shadow-base': shadow(
    `0 2px 8px ${alpha(BLACK, '0.06')}`,
    `0 1px 3px ${alpha(BLACK, '0.04')}`,
  ),
  '--shadow-lg': shadow(
    `0 4px 16px ${alpha(BLACK, '0.08')}`,
    `0 2px 6px ${alpha(BLACK, '0.04')}`,
  ),
  '--shadow-xl': shadow(
    `0 8px 24px ${alpha(BLACK, '0.1')}`,
    `0 4px 8px ${alpha(BLACK, '0.04')}`,
  ),
  '--shadow-2xl': shadow(
    `0 12px 32px ${alpha(BLACK, '0.12')}`,
    `0 4px 12px ${alpha(BLACK, '0.06')}`,
  ),

  '--blur-subtle': 'blur(4px) saturate(1.2)',
  '--blur-base': 'blur(8px) saturate(1.4)',
  '--blur-medium': 'blur(12px) saturate(1.6)',
  '--blur-strong': 'blur(16px) saturate(1.8)',
  '--blur-intense': 'blur(20px) saturate(2.0)',

  ...commonMobileThemeVars,

  '--opacity-disabled': '0.55',
  '--opacity-hover': '0.75',
  '--opacity-focus': '0.9',
  '--opacity-overlay': '0.35',

  '--scrollbar-thumb': alpha(BLACK, '0.15'),
  '--scrollbar-thumb-hover': alpha(BLACK, '0.25'),

  '--btn-default-bg': alpha(IOS_NEUTRAL, '0.08'),
  '--btn-default-color': TEXT_SECONDARY,
  '--btn-hover-bg': alpha(IOS_NEUTRAL, '0.14'),
  '--btn-hover-color': TEXT_PRIMARY,
  '--btn-primary-bg': ACCENT_500,
  '--btn-primary-color': BG_SECONDARY,
  '--btn-primary-hover-bg': ACCENT_600,
  '--btn-ghost-bg': TRANSPARENT,
  '--btn-ghost-color': TEXT_SECONDARY,
  '--btn-ghost-hover-bg': alpha(IOS_NEUTRAL, '0.08'),
};
