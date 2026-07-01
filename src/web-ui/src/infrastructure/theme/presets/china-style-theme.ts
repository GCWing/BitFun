

import { ThemeConfig } from '../types';
import {
  createChinaTypography,
  createCompactRadius,
  createGitColors,
  createStandardEasing,
  createStandardSpacing,
  createWindowControls,
  rgbFromHex,
  rgbaFromHex,
  STATIC_BLACK,
  STATIC_WHITE,
} from './shared';

const CHINA_STYLE_PAPER = '#faf8f0';
const CHINA_STYLE_INK = '#1a1a1a';
const CHINA_STYLE_BUTTON_TEXT = '#3a3a3a';
const CHINA_STYLE_BLUE = '#2e5e8a';
const CHINA_STYLE_BLUE_HOVER = '#234a6d';
const CHINA_STYLE_GREEN = '#7eb09b';
const CHINA_STYLE_GREEN_HOVER = '#5a9078';
const CHINA_STYLE_SUCCESS = '#52ad5a';
const CHINA_STYLE_WARNING = '#f0a020';
const CHINA_STYLE_ERROR = '#c8102e';
const CHINA_STYLE_BORDER = '#6a5c46';

const chinaStylePaper = (alpha: number | string) => rgbaFromHex(CHINA_STYLE_PAPER, alpha);
const chinaStyleBlue = (alpha: number | string) => rgbaFromHex(CHINA_STYLE_BLUE, alpha);
const chinaStyleBlueHover = (alpha: number | string) => rgbaFromHex(CHINA_STYLE_BLUE_HOVER, alpha);
const chinaStyleGreen = (alpha: number | string) => rgbaFromHex(CHINA_STYLE_GREEN, alpha);
const chinaStyleGreenHover = (alpha: number | string) => rgbaFromHex(CHINA_STYLE_GREEN_HOVER, alpha);
const chinaStyleSuccess = (alpha: number | string) => rgbaFromHex(CHINA_STYLE_SUCCESS, alpha);
const chinaStyleWarning = (alpha: number | string) => rgbaFromHex(CHINA_STYLE_WARNING, alpha);
const chinaStyleError = (alpha: number | string) => rgbaFromHex(CHINA_STYLE_ERROR, alpha);
const chinaStyleBorder = (alpha: number | string) => rgbaFromHex(CHINA_STYLE_BORDER, alpha);

export const bitfunChinaStyleTheme: ThemeConfig = {

  id: 'bitfun-china-style',
  name: 'Ink Charm',
  type: 'light',
  description: 'Chinese style theme - Rice paper and ink, blue and vermilion, warm and elegant',
  author: 'BitFun Team',
  version: '1.0.0',


  colors: {
    background: {
      primary: CHINA_STYLE_PAPER,
      secondary: '#f5f3e8',
      tertiary: '#f0ede0',
      quaternary: '#ebe8d8',
      elevated: '#ebe9e3',
      workbench: CHINA_STYLE_PAPER,
      scene: '#fdfcf6',
      tooltip: chinaStylePaper(0.96),
    },

    text: {
      primary: CHINA_STYLE_INK,
      secondary: '#3d3d3d',
      muted: '#6a6a6a',
      disabled: '#9a9a9a',
    },

    accent: {
      50: chinaStyleBlue(0.04),
      100: chinaStyleBlue(0.08),
      200: chinaStyleBlue(0.15),
      300: chinaStyleBlue(0.25),
      400: chinaStyleBlue(0.4),
      500: CHINA_STYLE_BLUE,
      600: CHINA_STYLE_BLUE_HOVER,
      700: chinaStyleBlueHover(0.8),
      800: chinaStyleBlueHover(0.9),
    },

    purple: {
      50: chinaStyleGreen(0.04),
      100: chinaStyleGreen(0.08),
      200: chinaStyleGreen(0.15),
      400: chinaStyleGreen(0.4),
      500: CHINA_STYLE_GREEN,
      600: CHINA_STYLE_GREEN_HOVER,
      800: chinaStyleGreenHover(0.9),
    },

    semantic: {
      success: CHINA_STYLE_SUCCESS,
      successBg: chinaStyleSuccess(0.08),
      successBorder: chinaStyleSuccess(0.25),

      warning: CHINA_STYLE_WARNING,
      warningBg: chinaStyleWarning(0.08),
      warningBorder: chinaStyleWarning(0.25),

      error: CHINA_STYLE_ERROR,
      errorBg: chinaStyleError(0.08),
      errorBorder: chinaStyleError(0.25),

      info: CHINA_STYLE_BLUE,
      infoBg: chinaStyleBlue(0.08),
      infoBorder: chinaStyleBlue(0.25),
    },

    border: {
      subtle: chinaStyleBorder(0.12),
      base: chinaStyleBorder(0.2),
      medium: chinaStyleBorder(0.28),
      strong: chinaStyleBorder(0.36),
      prominent: chinaStyleBorder(0.48),
    },

    element: {
      subtle: chinaStyleBlue(0.03),
      soft: chinaStyleBlue(0.06),
      base: chinaStyleBlue(0.1),
      medium: chinaStyleBlue(0.14),
      strong: chinaStyleBlue(0.18),
      elevated: rgbaFromHex(STATIC_WHITE, 0.85),
    },

    git: createGitColors({
      branch: rgbFromHex(CHINA_STYLE_BLUE),
      branchBg: chinaStyleBlue(0.08),
      changes: rgbFromHex(CHINA_STYLE_WARNING),
      changesBg: chinaStyleWarning(0.08),
      added: rgbFromHex(CHINA_STYLE_SUCCESS),
      addedBg: chinaStyleSuccess(0.08),
      deleted: rgbFromHex(CHINA_STYLE_ERROR),
      deletedBg: chinaStyleError(0.08),
    }),
  },


  effects: {
    shadow: {
      xs: `0 1px 2px ${chinaStyleBorder(0.06)}`,
      sm: `0 2px 4px ${chinaStyleBorder(0.08)}`,
      base: `0 4px 8px ${chinaStyleBorder(0.1)}`,
      lg: `0 8px 16px ${chinaStyleBorder(0.12)}`,
      xl: `0 12px 24px ${chinaStyleBorder(0.15)}`,
    },

    blur: {
      subtle: 'blur(4px) saturate(1.03)',
      base: 'blur(8px) saturate(1.05)',
    },

    radius: createCompactRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.5,
      hover: 0.75,
      focus: 0.9,
    },
  },


  motion: {
    duration: {
      instant: '0.1s',
      fast: '0.2s',
      base: '0.35s',
      slow: '0.7s',
    },

    easing: createStandardEasing('cubic-bezier(0.25, 0.1, 0.25, 1)'),
  },


  typography: createChinaTypography(),


  components: {

    windowControls: createWindowControls(CHINA_STYLE_ERROR),

    button: {



      primary: {
        default: {
          background: STATIC_BLACK,
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: '#262626',
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: CHINA_STYLE_INK,
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },


      ghost: {
        default: {
          color: '#5a5a5a',
        },
        hover: {
          background: chinaStyleBlue(0.11),
          color: CHINA_STYLE_BUTTON_TEXT,
          border: 'transparent',
        },
      },
    },
  },


  monaco: {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '6a6a6a', fontStyle: 'italic' },
      { token: 'keyword', foreground: 'c8102e' },
      { token: 'string', foreground: '52ad5a' },
      { token: 'number', foreground: 'f0a020' },
      { token: 'type', foreground: '2e5e8a' },
      { token: 'class', foreground: '2e5e8a' },
      { token: 'function', foreground: '7eb09b' },
      { token: 'variable', foreground: '3d3d3d' },
      { token: 'constant', foreground: 'a0522d' },
      { token: 'operator', foreground: 'c8102e' },
      { token: 'tag', foreground: '2e5e8a' },
      { token: 'attribute.name', foreground: '7eb09b' },
      { token: 'attribute.value', foreground: '52ad5a' },
    ],
    colors: {
      background: CHINA_STYLE_PAPER,
      foreground: CHINA_STYLE_INK,
      lineHighlight: '#f5f3e8',
      selection: chinaStyleBlue(0.28),
      cursor: CHINA_STYLE_BLUE,

      'editor.selectionBackground': chinaStyleBlue(0.28),
      'editor.selectionForeground': CHINA_STYLE_INK,
      'editor.inactiveSelectionBackground': chinaStyleBlue(0.18),
      'editor.selectionHighlightBackground': chinaStyleBlue(0.2),
      'editor.selectionHighlightBorder': chinaStyleBlue(0.35),
      'editorCursor.foreground': CHINA_STYLE_BLUE,
      'editor.wordHighlightBackground': chinaStyleBlue(0.12),
      'editor.wordHighlightStrongBackground': chinaStyleBlue(0.22),
    },
  },
};
