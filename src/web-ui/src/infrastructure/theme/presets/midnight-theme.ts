

import { ThemeConfig } from '../types';
import {
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
} from './shared';

const MIDNIGHT_BACKGROUND = '#2b2d30';
const MIDNIGHT_TEXT_PRIMARY = '#bcbec4';
const MIDNIGHT_BUTTON_TEXT = '#afb1b5';
const MIDNIGHT_ACCENT = '#58a6ff';
const MIDNIGHT_ACCENT_HOVER = '#3b82f6';
const MIDNIGHT_PURPLE = '#9c78ff';
const MIDNIGHT_PURPLE_HOVER = '#8b5cf6';
const MIDNIGHT_SUCCESS = '#6aab73';
const MIDNIGHT_WARNING = '#e0a055';
const MIDNIGHT_ERROR = '#cc7f7a';
const MIDNIGHT_CONTROL_ERROR = '#ef4444';

const midnightBackground = (alpha: number | string) => rgbaFromHex(MIDNIGHT_BACKGROUND, alpha);
const midnightText = (alpha: number | string) => rgbaFromHex(MIDNIGHT_TEXT_PRIMARY, alpha);
const midnightAccent = (alpha: number | string) => rgbaFromHex(MIDNIGHT_ACCENT, alpha);
const midnightAccentHover = (alpha: number | string) => rgbaFromHex(MIDNIGHT_ACCENT_HOVER, alpha);
const midnightPurple = (alpha: number | string) => rgbaFromHex(MIDNIGHT_PURPLE, alpha);
const midnightPurpleHover = (alpha: number | string) => rgbaFromHex(MIDNIGHT_PURPLE_HOVER, alpha);
const midnightSuccess = (alpha: number | string) => rgbaFromHex(MIDNIGHT_SUCCESS, alpha);
const midnightWarning = (alpha: number | string) => rgbaFromHex(MIDNIGHT_WARNING, alpha);
const midnightError = (alpha: number | string) => rgbaFromHex(MIDNIGHT_ERROR, alpha);

export const bitfunMidnightTheme: ThemeConfig = {

  id: 'bitfun-midnight',
  name: 'Midnight',
  type: 'dark',
  description: 'Midnight gray dark theme - Professional and elegant, inspired by JetBrains IDE',
  author: 'BitFun Team',
  version: '1.0.0',


  colors: {
    background: {
      primary: MIDNIGHT_BACKGROUND,
      secondary: '#1e1f22',
      tertiary: '#313335',
      quaternary: '#3c3f41',
      elevated: MIDNIGHT_BACKGROUND,
      workbench: '#212121',
      scene: MIDNIGHT_BACKGROUND,
      tooltip: midnightBackground(0.94),
    },

    text: {
      primary: MIDNIGHT_TEXT_PRIMARY,
      secondary: '#9da0a8',
      muted: '#6f737a',
      disabled: '#4e5157',
    },

    accent: {
      50: midnightAccent(0.04),
      100: midnightAccent(0.08),
      200: midnightAccent(0.15),
      300: midnightAccent(0.25),
      400: midnightAccent(0.4),
      500: MIDNIGHT_ACCENT,
      600: MIDNIGHT_ACCENT_HOVER,
      700: midnightAccentHover(0.8),
      800: midnightAccentHover(0.9),
    },

    purple: {
      50: midnightPurple(0.04),
      100: midnightPurple(0.08),
      200: midnightPurple(0.15),
      400: midnightPurple(0.4),
      500: MIDNIGHT_PURPLE,
      600: MIDNIGHT_PURPLE_HOVER,
      800: midnightPurpleHover(0.9),
    },

    semantic: {
      success: MIDNIGHT_SUCCESS,
      successBg: midnightSuccess(0.1),
      successBorder: midnightSuccess(0.3),

      warning: MIDNIGHT_WARNING,
      warningBg: midnightWarning(0.1),
      warningBorder: midnightWarning(0.3),

      error: MIDNIGHT_ERROR,
      errorBg: midnightError(0.1),
      errorBorder: midnightError(0.3),

      info: MIDNIGHT_ACCENT,
      infoBg: midnightAccent(0.1),
      infoBorder: midnightAccent(0.3),


    },

    border: {
      subtle: overlayWhite(0.08),
      base: overlayWhite(0.14),
      medium: overlayWhite(0.2),
      strong: overlayWhite(0.26),
      prominent: overlayWhite(0.35),
    },

    element: {
      subtle: overlayWhite(0.04),
      soft: overlayWhite(0.06),
      base: overlayWhite(0.09),
      medium: overlayWhite(0.12),
      strong: overlayWhite(0.15),
      elevated: overlayWhite(0.18),
    },

    git: createGitColors({
      branch: rgbFromHex(MIDNIGHT_ACCENT),
      branchBg: midnightAccent(0.1),
      changes: rgbFromHex(MIDNIGHT_WARNING),
      changesBg: midnightWarning(0.1),
      added: rgbFromHex(MIDNIGHT_SUCCESS),
      addedBg: midnightSuccess(0.1),
      deleted: rgbFromHex(MIDNIGHT_ERROR),
      deletedBg: midnightError(0.1),
    }),
  },


  effects: {
    shadow: {
      xs: `0 1px 2px ${overlayBlack(0.8)}`,
      sm: `0 2px 4px ${overlayBlack(0.75)}`,
      base: `0 4px 8px ${overlayBlack(0.7)}`,
      lg: `0 8px 16px ${overlayBlack(0.65)}`,
      xl: `0 12px 24px ${overlayBlack(0.8)}`,
    },

    blur: {
      subtle: 'blur(4px) saturate(1.1)',
      base: 'blur(8px) saturate(1.2)',
    },

    radius: createStandardRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.5,
      hover: 0.8,
      focus: 0.9,
    },
  },


  motion: {
    duration: {
      instant: '0.1s',
      fast: '0.15s',
      base: '0.3s',
      slow: '0.6s',
    },

    easing: createStandardEasing(),
  },


  typography: createStandardTypography(),


  components: {

    windowControls: createWindowControls(MIDNIGHT_CONTROL_ERROR),

    button: {



      primary: {
        default: {
          background: midnightAccent(0.2),
          color: '#6aa8e8',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: midnightAccent(0.3),
          color: '#8fc0f0',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: midnightAccent(0.24),
          color: '#8fc0f0',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },


      ghost: {
        default: {
          color: '#9a9a9a',
        },
        hover: {
          background: midnightText(0.13),
          color: MIDNIGHT_BUTTON_TEXT,
          border: 'transparent',
        },
      },
    },
  },


  monaco: {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '6f737a', fontStyle: 'italic' },
      { token: 'keyword', foreground: 'cc7832' },
      { token: 'string', foreground: '6aab73' },
      { token: 'number', foreground: '6897bb' },
      { token: 'type', foreground: 'e0a055' },
      { token: 'class', foreground: 'e0a055' },
      { token: 'function', foreground: 'ffc66d' },
      { token: 'variable', foreground: 'bcbec4' },
      { token: 'constant', foreground: '9876aa' },
      { token: 'operator', foreground: 'cc7832' },
      { token: 'tag', foreground: 'e8bf6a' },
      { token: 'attribute.name', foreground: 'bababa' },
      { token: 'attribute.value', foreground: 'a5c261' },
    ],
    colors: {
      background: MIDNIGHT_BACKGROUND,
      foreground: MIDNIGHT_TEXT_PRIMARY,
      lineHighlight: '#313335',
      selection: '#3d4752',
      cursor: MIDNIGHT_ACCENT,
    },
  },
};
