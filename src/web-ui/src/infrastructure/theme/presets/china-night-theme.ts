

import { ThemeConfig } from '../types';
import {
  createAccentScale,
  createChinaTypography,
  createCompactRadius,
  createGitColors,
  createSemanticColors,
  createSecondaryAccentScale,
  createStandardEasing,
  createStandardSpacing,
  createWindowControls,
  overlayBlack,
  rgbFromHex,
  rgbaFromHex,
} from './shared';

const CHINA_NIGHT_BACKGROUND = '#1c1c1f';
const CHINA_NIGHT_BACKGROUND_SECONDARY = '#212019';
const CHINA_NIGHT_TEXT_PRIMARY = '#e8e8e8';
const CHINA_NIGHT_BUTTON_TEXT = '#c5c3be';
const CHINA_NIGHT_ACCENT = '#73a5cc';
const CHINA_NIGHT_ACCENT_HOVER = '#5a8bb3';
const CHINA_NIGHT_GREEN = '#96c6b4';
const CHINA_NIGHT_GREEN_HOVER = '#7eb09b';
const CHINA_NIGHT_SUCCESS = '#6bc072';
const CHINA_NIGHT_WARNING = '#f5b555';
const CHINA_NIGHT_ERROR = '#e85555';

const chinaNightBackground = (alpha: number | string) => rgbaFromHex(CHINA_NIGHT_BACKGROUND, alpha);
const chinaNightText = (alpha: number | string) => rgbaFromHex(CHINA_NIGHT_TEXT_PRIMARY, alpha);
const chinaNightAccent = (alpha: number | string) => rgbaFromHex(CHINA_NIGHT_ACCENT, alpha);
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
      secondary: CHINA_NIGHT_BACKGROUND_SECONDARY,
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
      disabled: '#555555',
    },

    accent: createAccentScale({ base: CHINA_NIGHT_ACCENT, hover: CHINA_NIGHT_ACCENT_HOVER }),

    purple: createSecondaryAccentScale({ base: CHINA_NIGHT_GREEN, hover: CHINA_NIGHT_GREEN_HOVER }),

    semantic: createSemanticColors({
      success: CHINA_NIGHT_SUCCESS,
      warning: CHINA_NIGHT_WARNING,
      error: CHINA_NIGHT_ERROR,
      info: CHINA_NIGHT_ACCENT,
      bgAlpha: 0.12,
    }),

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
    },

    blur: {
      subtle: 'blur(4px) saturate(1.1)',
      base: 'blur(8px) saturate(1.15)',
    },

    radius: createCompactRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.45,
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

    windowControls: createWindowControls(CHINA_NIGHT_ERROR),

    button: {



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
          color: '#9a9a9a',
        },
        hover: {
          background: chinaNightAccent(0.13),
          color: CHINA_NIGHT_BUTTON_TEXT,
          border: 'transparent',
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
      lineHighlight: CHINA_NIGHT_BACKGROUND_SECONDARY,
      selection: chinaNightAccent(0.25),
      cursor: CHINA_NIGHT_ACCENT,
      'editor.selectionBackground': chinaNightAccent(0.25),
      'editorCursor.foreground': CHINA_NIGHT_ACCENT,
    },
  },
};
