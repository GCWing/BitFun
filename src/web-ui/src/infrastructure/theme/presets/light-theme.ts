

import { ThemeConfig } from '../types';
import {
  createAccentScale,
  createGitColors,
  createSemanticColors,
  createSecondaryAccentScale,
  createStandardEasing,
  createStandardRadius,
  createStandardSpacing,
  createStandardTypography,
  createWindowControls,
  rgbFromHex,
  rgbaFromHex,
  STATIC_BLACK,
  STATIC_WHITE,
} from './shared';

const LIGHT_INK = '#0f172a';
const LIGHT_TEXT_PRIMARY = '#1e293b';
const LIGHT_TEXT_STRONG = '#334155';
const LIGHT_ACCENT = '#64748b';
const LIGHT_ACCENT_HOVER = '#475569';
const LIGHT_PURPLE = '#7c6b99';
const LIGHT_PURPLE_HOVER = '#655680';
const LIGHT_SUCCESS = '#5b9a6f';
const LIGHT_WARNING = '#c08c42';
const LIGHT_ERROR = '#c26565';

const lightInk = (alpha: number | string) => rgbaFromHex(LIGHT_INK, alpha);
const lightAccent = (alpha: number | string) => rgbaFromHex(LIGHT_ACCENT, alpha);
const lightAccentHover = (alpha: number | string) => rgbaFromHex(LIGHT_ACCENT_HOVER, alpha);
const lightSuccess = (alpha: number | string) => rgbaFromHex(LIGHT_SUCCESS, alpha);
const lightWarning = (alpha: number | string) => rgbaFromHex(LIGHT_WARNING, alpha);
const lightError = (alpha: number | string) => rgbaFromHex(LIGHT_ERROR, alpha);

export const bitfunLightTheme: ThemeConfig = {

  id: 'bitfun-light',
  name: 'Light',
  type: 'light',
  description: 'Light theme - Neutral gray surfaces, black primary actions',
  author: 'BitFun Team',
  version: '2.3.0',

  layout: {
    sceneViewportBorder: false,
  },


  colors: {
    background: {
      primary: '#f3f3f5',
      secondary: STATIC_WHITE,
      tertiary: '#e8e8e8',
      quaternary: '#e8e8e8',
      elevated: STATIC_WHITE,
      workbench: '#e8e8e8',
      scene: STATIC_WHITE,
      tooltip: rgbaFromHex(STATIC_WHITE, 0.98),
    },

    text: {
      primary: LIGHT_TEXT_PRIMARY,
      secondary: '#3d4f66',
      muted: LIGHT_ACCENT,
      disabled: '#94a3b8',
    },


    accent: createAccentScale({
      base: LIGHT_ACCENT,
      hover: LIGHT_ACCENT_HOVER,
      alpha: { 700: 0.88 },
      stops: {
        50: lightInk(0.04),
        100: lightInk(0.07),
        200: lightInk(0.1),
        300: lightInk(0.16),
        400: lightInk(0.26),
        800: rgbaFromHex(LIGHT_TEXT_STRONG, 0.94),
      },
    }),


    purple: createSecondaryAccentScale({
      base: '#6b5a89',
      hover: LIGHT_PURPLE_HOVER,
      alpha: { 200: 0.14, 400: 0.36 },
      stops: {
        500: LIGHT_PURPLE,
      },
    }),


    semantic: createSemanticColors({
      success: LIGHT_SUCCESS,
      warning: LIGHT_WARNING,
      error: LIGHT_ERROR,
      info: LIGHT_ACCENT,
      bgAlpha: 0.08,
      borderAlpha: 0.25,
      overrides: {
        infoBg: lightAccent(0.1),
        infoBorder: lightAccent(0.28),
      },
    }),


    border: {
      subtle: lightAccent(0.15),
      base: lightAccent(0.22),
      medium: lightAccent(0.32),
      strong: lightAccent(0.42),
      prominent: lightAccent(0.52),
    },


    element: {
      subtle: lightInk(0.045),
      soft: lightInk(0.065),
      base: lightInk(0.09),
      medium: lightInk(0.12),
      strong: lightInk(0.16),
      elevated: rgbaFromHex(STATIC_WHITE, 0.92),
    },


    git: createGitColors({
      branch: rgbFromHex(LIGHT_ACCENT_HOVER),
      branchBg: lightAccentHover(0.1),
      changes: rgbFromHex(LIGHT_WARNING),
      changesBg: lightWarning(0.08),
      added: rgbFromHex(LIGHT_SUCCESS),
      addedBg: lightSuccess(0.08),
      deleted: rgbFromHex(LIGHT_ERROR),
      deletedBg: lightError(0.08),
    }),
  },


  effects: {
    shadow: {

      xs: `0 1px 2px ${lightAccentHover(0.06)}`,
      sm: `0 2px 4px ${lightAccentHover(0.08)}`,
      base: `0 4px 8px ${lightAccentHover(0.1)}`,
      lg: `0 8px 16px ${lightAccentHover(0.12)}`,
      xl: `0 12px 24px ${lightAccentHover(0.14)}`,
    },


    blur: {
      subtle: 'blur(4px) saturate(1.02)',
      base: 'blur(8px) saturate(1.05)',
    },

    radius: createStandardRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.55,
      hover: 0.75,
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

    windowControls: createWindowControls('#a85555'),

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
          background: '#1c1c1f',
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },


      ghost: {
        default: {
          color: LIGHT_ACCENT_HOVER,
        },
        hover: {
          background: lightInk(0.08),
          color: LIGHT_TEXT_STRONG,
          border: 'transparent',
        },
      },
    },
  },


  monaco: {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '94a3b8', fontStyle: 'italic' },
      { token: 'keyword', foreground: '6b5a89' },
      { token: 'string', foreground: '5b9a6f' },
      { token: 'number', foreground: 'b8863a' },
      { token: 'type', foreground: '475569' },
      { token: 'class', foreground: '475569' },
      { token: 'function', foreground: '7c6b99' },
      { token: 'variable', foreground: '475569' },
      { token: 'constant', foreground: 'c08c42' },
      { token: 'operator', foreground: '6b5a89' },
      { token: 'tag', foreground: '475569' },
      { token: 'attribute.name', foreground: '7c6b99' },
      { token: 'attribute.value', foreground: '5b9a6f' },
    ],
    colors: {
      background: '#f3f3f5',
      foreground: LIGHT_TEXT_PRIMARY,
      lineHighlight: '#f0f4f8',
      selection: lightInk(0.14),
      cursor: LIGHT_TEXT_PRIMARY,

      'editor.selectionBackground': lightInk(0.14),
      'editor.selectionForeground': LIGHT_TEXT_PRIMARY,
      'editor.inactiveSelectionBackground': lightInk(0.09),
      'editor.selectionHighlightBackground': lightInk(0.1),
      'editor.selectionHighlightBorder': lightInk(0.22),
      'editorCursor.foreground': LIGHT_TEXT_PRIMARY,

      'editor.wordHighlightBackground': lightInk(0.07),
      'editor.wordHighlightStrongBackground': lightInk(0.11),
    },
  },
};




