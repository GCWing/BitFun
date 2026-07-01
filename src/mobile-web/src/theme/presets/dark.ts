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
const ACCENT = '96, 165, 250';
const PURPLE = '139, 92, 246';
const PINK = '236, 72, 153';
const SUCCESS = '52, 211, 153';
const WARNING = '245, 158, 11';
const ERROR = '239, 68, 68';
const INFO = '225, 171, 128';

const BG_PRIMARY = '#121214';
const BG_SECONDARY = '#18181a';
const BG_SCENE = BG_SECONDARY;
const BG_QUATERNARY = '#202024';
const TEXT_PRIMARY = '#e8e8e8';
const TEXT_SECONDARY = '#b0b0b0';
const TEXT_MUTED = '#858585';
const TEXT_DISABLED = '#555555';
const ACCENT_500 = '#60a5fa';
const ACCENT_600 = '#3b82f6';
const PURPLE_500 = '#8b5cf6';
const PURPLE_600 = '#7c3aed';
const PINK_500 = '#ec4899';
const PINK_600 = '#db2777';
const SUCCESS_500 = '#34d399';
const WARNING_500 = '#f59e0b';
const ERROR_500 = '#ef4444';
const INFO_500 = '#e1ab80';
const INFO_HOVER = '#f6d0a3';

export const darkTheme: MobileThemeVars = {
  '--color-bg-primary': BG_PRIMARY,
  '--color-bg-secondary': BG_SECONDARY,
  '--color-bg-tertiary': BG_PRIMARY,
  '--color-bg-quaternary': BG_QUATERNARY,
  '--color-bg-elevated': BG_SECONDARY,
  '--color-bg-workbench': BG_PRIMARY,
  '--color-bg-scene': BG_SCENE,
  '--color-bg-flowchat': BG_SCENE,
  '--color-bg-tooltip': alpha('30, 30, 32', '0.92'),

  '--color-text-primary': TEXT_PRIMARY,
  '--color-text-secondary': TEXT_SECONDARY,
  '--color-text-muted': TEXT_MUTED,
  '--color-text-disabled': TEXT_DISABLED,

  '--element-bg-subtle': alpha(WHITE, '0.05'),
  '--element-bg-soft': alpha(WHITE, '0.095'),
  '--element-bg-base': alpha(WHITE, '0.125'),
  '--element-bg-medium': alpha(WHITE, '0.155'),
  '--element-bg-strong': alpha(WHITE, '0.19'),
  '--element-bg-elevated': alpha(WHITE, '0.24'),

  ...colorRamp('--color-accent', ACCENT, ACCENT_500, ACCENT_600),
  ...colorRamp('--color-purple', PURPLE, PURPLE_500, PURPLE_600),
  ...colorRamp('--color-pink', PINK, PINK_500, PINK_600),

  '--color-success': SUCCESS_500,
  '--color-success-bg': alpha(SUCCESS, '0.1'),
  '--color-success-border': alpha(SUCCESS, '0.3'),
  '--color-warning': WARNING_500,
  '--color-warning-bg': alpha(WARNING, '0.1'),
  '--color-warning-border': alpha(WARNING, '0.3'),
  '--color-error': ERROR_500,
  '--color-error-bg': alpha(ERROR, '0.1'),
  '--color-error-border': alpha(ERROR, '0.3'),
  '--color-info': INFO_500,
  '--color-info-bg': alpha(INFO, '0.1'),
  '--color-info-border': alpha(INFO, '0.3'),

  '--color-highlight': INFO_500,
  '--color-highlight-bg': alpha(INFO, '0.15'),
  '--color-overlay': alpha(BLACK, '0.5'),

  '--border-subtle': alpha(WHITE, '0.12'),
  '--border-base': alpha(WHITE, '0.18'),
  '--border-medium': alpha(WHITE, '0.24'),
  '--border-strong': alpha(WHITE, '0.32'),
  '--border-prominent': alpha(INFO, '0.5'),

  '--shadow-xs': `0 1px 2px ${alpha(BLACK, '0.9')}`,
  '--shadow-sm': `0 2px 4px ${alpha(BLACK, '0.8')}`,
  '--shadow-base': `0 4px 8px ${alpha(BLACK, '0.7')}`,
  '--shadow-lg': `0 8px 16px ${alpha(BLACK, '0.6')}`,
  '--shadow-xl': `0 12px 24px ${alpha(BLACK, '0.5')}`,

  '--blur-subtle': 'blur(4px) saturate(1.05)',
  '--blur-base': 'blur(8px) saturate(1.1)',

  ...commonMobileThemeVars,

  '--opacity-disabled': '0.6',
  '--opacity-hover': '0.8',
  '--opacity-focus': '0.9',

  '--scrollbar-thumb': alpha(WHITE, '0.15'),
  '--scrollbar-thumb-hover': alpha(WHITE, '0.24'),

  '--btn-default-bg': alpha(WHITE, '0.08'),
  '--btn-default-color': TEXT_MUTED,
  '--btn-hover-bg': alpha(WHITE, '0.12'),
  '--btn-hover-color': TEXT_SECONDARY,
  '--btn-primary-bg': INFO_500,
  '--btn-primary-color': '#000000',
  '--btn-primary-hover-bg': INFO_HOVER,
  '--btn-ghost-bg': TRANSPARENT,
  '--btn-ghost-color': TEXT_MUTED,
  '--btn-ghost-hover-bg': alpha(WHITE, '0.095'),
};
