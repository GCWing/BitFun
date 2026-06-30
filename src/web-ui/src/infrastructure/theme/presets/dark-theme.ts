

import { ThemeConfig } from '../types';
import {
  createDarkNeutralBorder,
  createDarkNeutralElement,
  createDarkNeutralScrollbar,
  createGitColors,
  createStandardEasing,
  createStandardRadius,
  createStandardSpacing,
  createStandardTypography,
  createWindowControls,
  overlayBlack,
  overlayWhite,
  rgbFromHex,
  rgbaFromHex,
  STATIC_WHITE,
} from './shared';

const DARK_BACKGROUND_PRIMARY = '#0e0e10';
const DARK_BACKGROUND_SECONDARY = '#1c1c1f';
const DARK_TEXT_PRIMARY = '#e8e8e8';
const DARK_BUTTON_TEXT = '#c8c8c8';
const DARK_ACCENT = '#60a5fa';
const DARK_ACCENT_HOVER = '#3b82f6';
const DARK_PURPLE = '#8b5cf6';
const DARK_PURPLE_HOVER = '#7c3aed';
const DARK_SUCCESS = '#34d399';
const DARK_WARNING = '#f59e0b';
const DARK_ERROR = '#ef4444';

const darkAccent = (alpha: number | string) => rgbaFromHex(DARK_ACCENT, alpha);
const darkAccentHover = (alpha: number | string) => rgbaFromHex(DARK_ACCENT_HOVER, alpha);
const darkPurple = (alpha: number | string) => rgbaFromHex(DARK_PURPLE, alpha);
const darkPurpleHover = (alpha: number | string) => rgbaFromHex(DARK_PURPLE_HOVER, alpha);
const darkSuccess = (alpha: number | string) => rgbaFromHex(DARK_SUCCESS, alpha);
const darkWarning = (alpha: number | string) => rgbaFromHex(DARK_WARNING, alpha);
const darkError = (alpha: number | string) => rgbaFromHex(DARK_ERROR, alpha);

export const bitfunDarkTheme: ThemeConfig = {

  id: 'bitfun-dark',
  name: 'Dark',
  type: 'dark',
  description: 'Default dark theme',
  author: 'BitFun Team',
  version: '2.1.0',


  colors: {
    background: {
      primary: DARK_BACKGROUND_PRIMARY,
      secondary: DARK_BACKGROUND_SECONDARY,
      tertiary: DARK_BACKGROUND_PRIMARY,
      quaternary: '#252528',
      elevated: DARK_BACKGROUND_SECONDARY,
      workbench: DARK_BACKGROUND_PRIMARY,
      scene: DARK_BACKGROUND_SECONDARY,
      tooltip: 'rgba(28, 28, 31, 0.96)',
    },

    text: {
      primary: DARK_TEXT_PRIMARY,
      secondary: '#b0b0b0',
      muted: '#858585',
      disabled: '#555555',
    },

    accent: {
      50: darkAccent(0.04),
      100: darkAccent(0.08),
      200: darkAccent(0.15),
      300: darkAccent(0.25),
      400: darkAccent(0.4),
      500: DARK_ACCENT,
      600: DARK_ACCENT_HOVER,
      700: darkAccentHover(0.8),
      800: darkAccentHover(0.9),
    },

    purple: {
      50: darkPurple(0.04),
      100: darkPurple(0.08),
      200: darkPurple(0.15),
      300: darkPurple(0.25),
      400: darkPurple(0.4),
      500: DARK_PURPLE,
      600: DARK_PURPLE_HOVER,
      700: darkPurpleHover(0.8),
      800: darkPurpleHover(0.9),
    },

    semantic: {
      success: DARK_SUCCESS,
      successBg: darkSuccess(0.1),
      successBorder: darkSuccess(0.3),

      warning: DARK_WARNING,
      warningBg: darkWarning(0.1),
      warningBorder: darkWarning(0.3),

      error: DARK_ERROR,
      errorBg: darkError(0.1),
      errorBorder: darkError(0.3),

      info: '#a1a1aa',
      infoBg: overlayWhite(0.08),
      infoBorder: overlayWhite(0.24),


      highlight: '#a8a8a8',
      highlightBg: overlayWhite(0.1),
    },

    border: createDarkNeutralBorder(),

    element: createDarkNeutralElement(),

    git: createGitColors({
      branch: '#a1a1aa',
      branchBg: overlayWhite(0.06),
      changes: rgbFromHex(DARK_WARNING),
      changesBg: darkWarning(0.1),
      added: 'rgb(34, 197, 94)',
      addedBg: 'rgba(34, 197, 94, 0.1)',
      deleted: rgbFromHex(DARK_ERROR),
      deletedBg: darkError(0.1),
    }),

    scrollbar: createDarkNeutralScrollbar(),
  },


  effects: {
    shadow: {
      xs: `0 1px 2px ${overlayBlack(0.9)}`,
      sm: `0 2px 4px ${overlayBlack(0.8)}`,
      base: `0 4px 8px ${overlayBlack(0.7)}`,
      lg: `0 8px 16px ${overlayBlack(0.6)}`,
      xl: `0 12px 24px ${overlayBlack(0.5)}`,
      '2xl': `0 16px 32px ${overlayBlack(0.4)}`,
    },

    glow: {
      blue: `0 12px 32px ${darkAccent(0.2)}, 0 6px 16px ${darkAccent(0.12)}, 0 3px 8px ${overlayBlack(0.12)}`,
      purple: `0 12px 32px ${darkPurple(0.22)}, 0 6px 16px ${darkPurpleHover(0.14)}, 0 3px 8px ${overlayBlack(0.12)}`,
      mixed: `0 12px 32px ${overlayWhite(0.06)}, 0 6px 16px ${darkPurple(0.12)}, 0 3px 8px ${overlayBlack(0.12)}`,
    },

    blur: {
      subtle: 'blur(4px) saturate(1.05)',
      base: 'blur(8px) saturate(1.1)',
      medium: 'blur(12px) saturate(1.2)',
      strong: 'blur(16px) saturate(1.3) brightness(1.1)',
      intense: 'blur(20px) saturate(1.4) brightness(1.15)',
    },

    radius: createStandardRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.6,
      hover: 0.8,
      focus: 0.9,
      overlay: 0.4,
    },
  },


  motion: {
    duration: {
      instant: '0.1s',
      fast: '0.15s',
      base: '0.3s',
      slow: '0.6s',
      lazy: '1s',
    },

    easing: createStandardEasing(),
  },


  typography: createStandardTypography(),


  components: {

    windowControls: createWindowControls({
      standard: {
        dot: overlayWhite(0.38),
        dotShadow: `0 0 4px ${overlayBlack(0.35)}`,
        hoverBg: overlayWhite(0.1),
        hoverColor: '#e4e4e4',
        hoverBorder: overlayWhite(0.16),
        hoverShadow: `0 2px 8px ${overlayBlack(0.25)}, inset 0 1px 0 ${overlayWhite(0.08)}`,
      },
      close: {
        dot: darkError(0.45),
        dotShadow: `0 0 4px ${darkError(0.2)}`,
        hoverBg: darkError(0.12),
        hoverColor: DARK_ERROR,
        hoverBorder: darkError(0.2),
        hoverShadow: `0 2px 8px ${darkError(0.15)}, inset 0 1px 0 ${overlayWhite(0.1)}`,
      },
      common: {
        defaultColor: rgbaFromHex(DARK_TEXT_PRIMARY, 0.9),
        defaultDot: overlayWhite(0.2),
        disabledDot: overlayWhite(0.1),
        flowGradient: `linear-gradient(90deg, transparent, ${overlayWhite(0.05)}, ${overlayWhite(0.08)}, ${overlayWhite(0.05)}, transparent)`,
      },
    }),

    button: {

      default: {
        background: overlayWhite(0.08),
        color: '#9a9a9a',
        border: 'transparent',
        shadow: 'none',
      },
      hover: {
        background: overlayWhite(0.14),
        color: DARK_BUTTON_TEXT,
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      active: {
        background: overlayWhite(0.1),
        color: DARK_BUTTON_TEXT,
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },


      primary: {
        default: {
          background: overlayWhite(0.16),
          color: '#f4f4f5',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: overlayWhite(0.24),
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: overlayWhite(0.2),
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },


      ghost: {
        default: {
          background: 'transparent',
          color: '#9a9a9a',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: overlayWhite(0.1),
          color: DARK_BUTTON_TEXT,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: overlayWhite(0.07),
          color: DARK_BUTTON_TEXT,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
    },
  },




  monaco: {
    base: 'vs-dark',
    inherit: true,
    rules: [],
    colors: {
      background: '#121214',
      foreground: DARK_TEXT_PRIMARY,
      lineHighlight: '#18181a',
      selection: overlayWhite(0.12),
      cursor: '#c4c4c4',
    },
  },
};





