

import { ThemeConfig } from '../types';
import {
  createChinaTypography,
  createCompactRadius,
  createGitColors,
  createStandardEasing,
  createStandardSpacing,
  createWindowControls,
  overlayBlack,
  overlayWhite,
  rgbFromHex,
  rgbaFromHex,
} from './shared';

const CHINA_NIGHT_BACKGROUND = '#1a1a1a';
const CHINA_NIGHT_TEXT_PRIMARY = '#e8e6e1';
const CHINA_NIGHT_BUTTON_TEXT = '#ccc9c4';
const CHINA_NIGHT_ACCENT = '#73a5cc';
const CHINA_NIGHT_ACCENT_HOVER = '#5a8bb3';
const CHINA_NIGHT_GREEN = '#96c6b4';
const CHINA_NIGHT_GREEN_HOVER = '#7aab98';
const CHINA_NIGHT_SUCCESS = '#6bc072';
const CHINA_NIGHT_WARNING = '#f5b555';
const CHINA_NIGHT_ERROR = '#e85555';
const CHINA_NIGHT_HIGHLIGHT = '#e6a84a';

const chinaNightBackground = (alpha: number | string) => rgbaFromHex(CHINA_NIGHT_BACKGROUND, alpha);
const chinaNightText = (alpha: number | string) => rgbaFromHex(CHINA_NIGHT_TEXT_PRIMARY, alpha);
const chinaNightAccent = (alpha: number | string) => rgbaFromHex(CHINA_NIGHT_ACCENT, alpha);
const chinaNightAccentHover = (alpha: number | string) => rgbaFromHex(CHINA_NIGHT_ACCENT_HOVER, alpha);
const chinaNightGreen = (alpha: number | string) => rgbaFromHex(CHINA_NIGHT_GREEN, alpha);
const chinaNightGreenHover = (alpha: number | string) => rgbaFromHex(CHINA_NIGHT_GREEN_HOVER, alpha);
const chinaNightSuccess = (alpha: number | string) => rgbaFromHex(CHINA_NIGHT_SUCCESS, alpha);
const chinaNightWarning = (alpha: number | string) => rgbaFromHex(CHINA_NIGHT_WARNING, alpha);
const chinaNightError = (alpha: number | string) => rgbaFromHex(CHINA_NIGHT_ERROR, alpha);

export const bitfunChinaNightTheme: ThemeConfig = {

  id: 'bitfun-china-night',
  name: 'Ink Night',
  type: 'dark',
  description: 'Chinese dark theme - Starlit ink night, moonlight like water, serene and elegant',
  author: 'BitFun Team',
  version: '1.0.0',


  colors: {
    background: {
      primary: CHINA_NIGHT_BACKGROUND,
      secondary: '#212019',
      tertiary: '#262626',
      quaternary: '#262626',
      elevated: '#262626',
      workbench: CHINA_NIGHT_BACKGROUND,
      scene: CHINA_NIGHT_BACKGROUND,
      tooltip: chinaNightBackground(0.95),
    },

    text: {
      primary: CHINA_NIGHT_TEXT_PRIMARY,
      secondary: '#c5c3be',
      muted: '#928f89',
      disabled: '#5f5d59',
    },

    accent: {
      50: chinaNightAccent(0.04),
      100: chinaNightAccent(0.08),
      200: chinaNightAccent(0.15),
      300: chinaNightAccent(0.25),
      400: chinaNightAccent(0.4),
      500: CHINA_NIGHT_ACCENT,
      600: CHINA_NIGHT_ACCENT_HOVER,
      700: chinaNightAccentHover(0.8),
      800: chinaNightAccentHover(0.9),
    },

    purple: {
      50: chinaNightGreen(0.04),
      100: chinaNightGreen(0.08),
      200: chinaNightGreen(0.15),
      300: chinaNightGreen(0.25),
      400: chinaNightGreen(0.4),
      500: CHINA_NIGHT_GREEN,
      600: CHINA_NIGHT_GREEN_HOVER,
      700: chinaNightGreenHover(0.8),
      800: chinaNightGreenHover(0.9),
    },

    semantic: {
      success: CHINA_NIGHT_SUCCESS,
      successBg: chinaNightSuccess(0.12),
      successBorder: chinaNightSuccess(0.3),

      warning: CHINA_NIGHT_WARNING,
      warningBg: chinaNightWarning(0.12),
      warningBorder: chinaNightWarning(0.3),

      error: CHINA_NIGHT_ERROR,
      errorBg: chinaNightError(0.12),
      errorBorder: chinaNightError(0.3),

      info: CHINA_NIGHT_ACCENT,
      infoBg: chinaNightAccent(0.12),
      infoBorder: chinaNightAccent(0.3),


      highlight: CHINA_NIGHT_HIGHLIGHT,
      highlightBg: rgbaFromHex(CHINA_NIGHT_HIGHLIGHT, 0.15),
    },

    border: {
      subtle: chinaNightText(0.1),
      base: chinaNightText(0.16),
      medium: chinaNightText(0.22),
      strong: chinaNightText(0.28),
      prominent: chinaNightText(0.38),
    },

    element: {
      subtle: chinaNightAccent(0.06),
      soft: chinaNightAccent(0.09),
      base: chinaNightAccent(0.12),
      medium: chinaNightAccent(0.16),
      strong: chinaNightAccent(0.2),
      elevated: rgbaFromHex('#262626', 0.95),
    },

    git: createGitColors({
      branch: rgbFromHex(CHINA_NIGHT_ACCENT),
      branchBg: chinaNightAccent(0.12),
      changes: rgbFromHex(CHINA_NIGHT_WARNING),
      changesBg: chinaNightWarning(0.12),
      added: rgbFromHex(CHINA_NIGHT_SUCCESS),
      addedBg: chinaNightSuccess(0.12),
      deleted: rgbFromHex(CHINA_NIGHT_ERROR),
      deletedBg: chinaNightError(0.12),
    }),
  },


  effects: {
    shadow: {
      xs: `0 1px 2px ${overlayBlack(0.5)}`,
      sm: `0 2px 4px ${overlayBlack(0.6)}`,
      base: `0 4px 8px ${overlayBlack(0.65)}`,
      lg: `0 8px 16px ${overlayBlack(0.7)}`,
      xl: `0 12px 24px ${overlayBlack(0.75)}`,
      '2xl': `0 16px 32px ${overlayBlack(0.8)}`,
    },

    glow: {
      blue: `0 8px 24px ${chinaNightAccent(0.25)}, 0 4px 12px ${chinaNightAccent(0.18)}, 0 2px 6px ${overlayBlack(0.3)}`,
      purple: `0 8px 24px ${chinaNightGreen(0.25)}, 0 4px 12px ${chinaNightGreen(0.18)}, 0 2px 6px ${overlayBlack(0.3)}`,
      mixed: `0 8px 24px ${chinaNightAccent(0.2)}, 0 4px 12px ${chinaNightGreen(0.18)}, 0 2px 6px ${overlayBlack(0.3)}`,
    },

    blur: {
      subtle: 'blur(4px) saturate(1.1)',
      base: 'blur(8px) saturate(1.15)',
      medium: 'blur(12px) saturate(1.2)',
      strong: 'blur(16px) saturate(1.25) brightness(1.05)',
      intense: 'blur(20px) saturate(1.3) brightness(1.08)',
    },

    radius: createCompactRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.45,
      hover: 0.75,
      focus: 0.9,
      overlay: 0.5,
    },
  },


  motion: {
    duration: {
      instant: '0.1s',
      fast: '0.2s',
      base: '0.35s',
      slow: '0.7s',
      lazy: '1.2s',
    },

    easing: createStandardEasing('cubic-bezier(0.25, 0.1, 0.25, 1)'),
  },


  typography: createChinaTypography(),


  components: {

    windowControls: createWindowControls({
      standard: {
        dot: chinaNightAccent(0.45),
        dotShadow: `0 0 4px ${chinaNightAccent(0.2)}`,
        hoverBg: chinaNightAccent(0.12),
        hoverColor: CHINA_NIGHT_ACCENT,
        hoverBorder: chinaNightAccent(0.2),
        hoverShadow: `0 2px 8px ${chinaNightAccent(0.15)}, inset 0 1px 0 ${overlayWhite(0.1)}`,
      },
      close: {
        dot: chinaNightError(0.45),
        dotShadow: `0 0 4px ${chinaNightError(0.2)}`,
        hoverBg: chinaNightError(0.12),
        hoverColor: CHINA_NIGHT_ERROR,
        hoverBorder: chinaNightError(0.2),
        hoverShadow: `0 2px 8px ${chinaNightError(0.15)}, inset 0 1px 0 ${overlayWhite(0.1)}`,
      },
      common: {
        defaultColor: overlayWhite(0.9),
        defaultDot: overlayWhite(0.2),
        disabledDot: overlayWhite(0.1),
        flowGradient: `linear-gradient(90deg, transparent, ${overlayWhite(0.05)}, ${overlayWhite(0.08)}, ${overlayWhite(0.05)}, transparent)`,
      },
    }),

    button: {

      default: {
        background: chinaNightAccent(0.11),
        color: '#9a9a9a',
        border: 'transparent',
        shadow: 'none',
      },
      hover: {
        background: chinaNightAccent(0.19),
        color: CHINA_NIGHT_BUTTON_TEXT,
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      active: {
        background: chinaNightAccent(0.15),
        color: CHINA_NIGHT_BUTTON_TEXT,
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },


      primary: {
        default: {
          background: chinaNightAccent(0.24),
          color: '#88b8d8',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: chinaNightAccent(0.34),
          color: '#b0d5ea',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: chinaNightAccent(0.28),
          color: '#b0d5ea',
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
          background: chinaNightAccent(0.13),
          color: CHINA_NIGHT_BUTTON_TEXT,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: chinaNightAccent(0.11),
          color: CHINA_NIGHT_BUTTON_TEXT,
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
    rules: [
      { token: 'comment', foreground: '928f89', fontStyle: 'italic' },
      { token: 'keyword', foreground: 'e85555' },
      { token: 'string', foreground: '6bc072' },
      { token: 'number', foreground: 'f5b555' },
      { token: 'type', foreground: '73a5cc' },
      { token: 'class', foreground: '73a5cc' },
      { token: 'function', foreground: '96c6b4' },
      { token: 'variable', foreground: 'c5c3be' },
      { token: 'constant', foreground: 'd4a574' },
      { token: 'operator', foreground: 'e85555' },
      { token: 'tag', foreground: '73a5cc' },
      { token: 'attribute.name', foreground: '96c6b4' },
      { token: 'attribute.value', foreground: '6bc072' },
    ],
    colors: {
      background: CHINA_NIGHT_BACKGROUND,
      foreground: CHINA_NIGHT_TEXT_PRIMARY,
      lineHighlight: '#212019',
      selection: chinaNightAccent(0.25),
      cursor: CHINA_NIGHT_ACCENT,
      'editor.selectionBackground': chinaNightAccent(0.25),
      'editorCursor.foreground': CHINA_NIGHT_ACCENT,
    },
  },
};

