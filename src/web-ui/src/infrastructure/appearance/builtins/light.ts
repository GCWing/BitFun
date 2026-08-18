

import { AppearancePalette } from './AppearancePalette';
import {
  createAccentScale,
  createGitColors,
  createSemanticColors,
  createSecondaryAccentScale,
  createStandardEasing,
  createStandardRadius,
  createStandardSpacing,
  createStandardTypography,
  rgbFromHex,
  rgbaFromHex,
  STATIC_BLACK,
  STATIC_WHITE,
} from './paletteHelpers';

const LIGHT_NAVY = '#101a27';
const LIGHT_TEXT_PRIMARY = '#1c1c1f';
const LIGHT_TEXT_SECONDARY = '#555555';
const LIGHT_TEXT_MUTED = '#6a6a6a';
const LIGHT_TEXT_DISABLED = '#9a9a9a';
const LIGHT_NAVY_HOVER = LIGHT_TEXT_PRIMARY;
const LIGHT_PURPLE = '#7c6b99';
const LIGHT_PURPLE_HOVER = '#655680';
const LIGHT_SUCCESS = '#247344';
const LIGHT_SUCCESS_BG = '#e1fbe9';
const LIGHT_WARNING = '#9a651f';
const LIGHT_ERROR = '#a74352';
const LIGHT_ERROR_BG = rgbaFromHex(LIGHT_ERROR, 0.12);
const LIGHT_BACKGROUND_PRIMARY = '#fdfdfd';
const LIGHT_SURFACE_SUBTLE = rgbaFromHex(LIGHT_NAVY, 0.03);
const LIGHT_SURFACE_SOFT = '#f3f3f5';
const LIGHT_BORDER_BASE = rgbaFromHex(LIGHT_NAVY, 0.15);

const lightNavy = (alpha: number | string) => rgbaFromHex(LIGHT_NAVY, alpha);
const lightNavyHover = (alpha: number | string) => rgbaFromHex(LIGHT_NAVY_HOVER, alpha);

export const bitfunLightPalette: AppearancePalette = {

  id: 'bitfun-light',
  name: 'Light',
  type: 'light',
  description: 'Light appearance - Crisp white surfaces, soft neutral grays, deep navy actions',
  author: 'BitFun Team',
  version: '2.4.0',

  layout: {
    sceneViewportBorder: false,
  },


  colors: {
    background: {
      primary: LIGHT_BACKGROUND_PRIMARY,
      secondary: STATIC_WHITE,
      tertiary: LIGHT_SURFACE_SOFT,
      elevated: STATIC_WHITE,
      workbench: LIGHT_SURFACE_SOFT,
      scene: STATIC_WHITE,
    },

    text: {
      primary: LIGHT_TEXT_PRIMARY,
      secondary: LIGHT_TEXT_SECONDARY,
      muted: LIGHT_TEXT_MUTED,
      disabled: LIGHT_TEXT_DISABLED,
    },


    accent: createAccentScale({
      base: LIGHT_NAVY,
      hover: LIGHT_NAVY_HOVER,
      stops: {
        50: LIGHT_SURFACE_SUBTLE,
        100: LIGHT_SURFACE_SOFT,
        200: lightNavy(0.12),
        300: lightNavy(0.18),
        400: lightNavy(0.3),
        700: STATIC_BLACK,
      },
    }),


    purple: createSecondaryAccentScale({
      base: '#6b5a89',
      hover: LIGHT_PURPLE_HOVER,
      alpha: { 200: 0.14 },
      stops: {
        500: LIGHT_PURPLE,
      },
    }),


    semantic: createSemanticColors({
      success: LIGHT_SUCCESS,
      warning: LIGHT_WARNING,
      error: LIGHT_ERROR,
      info: LIGHT_TEXT_SECONDARY,
      bgAlpha: 0.08,
      borderAlpha: 0.25,
      overrides: {
        successBg: LIGHT_SUCCESS_BG,
        successBorder: LIGHT_SUCCESS,
        errorBg: LIGHT_ERROR_BG,
        errorBorder: rgbaFromHex(LIGHT_ERROR, 0.36),
        infoBg: LIGHT_SURFACE_SOFT,
        infoBorder: LIGHT_BORDER_BASE,
      },
    }),


    border: {
      subtle: lightNavy(0.08),
      base: LIGHT_BORDER_BASE,
      medium: lightNavy(0.24),
      strong: lightNavy(0.34),
      prominent: lightNavy(0.48),
    },


    element: {
      subtle: LIGHT_SURFACE_SUBTLE,
      soft: LIGHT_SURFACE_SOFT,
      base: lightNavy(0.09),
      medium: lightNavy(0.13),
      strong: lightNavy(0.18),
    },


    git: createGitColors({
      branch: rgbFromHex(LIGHT_NAVY_HOVER),
      branchBg: lightNavyHover(0.1),
      changes: rgbFromHex(LIGHT_WARNING),
      added: rgbFromHex(LIGHT_SUCCESS),
      deleted: rgbFromHex(LIGHT_ERROR),
    }),
  },


  effects: {
    shadow: {

      xs: `0 1px 2px ${lightNavy(0.04)}`,
      sm: `0 2px 4px ${lightNavy(0.055)}`,
      base: `0 4px 8px ${lightNavy(0.07)}`,
      lg: `0 8px 16px ${lightNavy(0.09)}`,
      xl: `0 12px 24px ${lightNavy(0.11)}`,
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
      instant: '0.08s',
      fast: '0.14s',
      base: '0.22s',
      slow: '0.42s',
    },

    easing: createStandardEasing(),
  },


  typography: createStandardTypography(),


  components: {
    button: {



      primary: {
        default: {
          background: LIGHT_NAVY,
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: LIGHT_NAVY_HOVER,
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: STATIC_BLACK,
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },


      ghost: {
        default: {
          color: LIGHT_TEXT_SECONDARY,
        },
        hover: {
          background: LIGHT_SURFACE_SOFT,
          color: LIGHT_TEXT_PRIMARY,
          border: 'transparent',
        },
      },
    },
  },


  monaco: {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '9a9a9a', fontStyle: 'italic' },
      { token: 'keyword', foreground: '6b5a89' },
      { token: 'string', foreground: '247344' },
      { token: 'number', foreground: '9a651f' },
      { token: 'type', foreground: '555555' },
      { token: 'class', foreground: '555555' },
      { token: 'function', foreground: '7c6b99' },
      { token: 'variable', foreground: '555555' },
      { token: 'constant', foreground: '9a651f' },
      { token: 'operator', foreground: '6b5a89' },
      { token: 'tag', foreground: '555555' },
      { token: 'attribute.name', foreground: '7c6b99' },
      { token: 'attribute.value', foreground: '247344' },
    ],
    colors: {
      background: STATIC_WHITE,
      foreground: LIGHT_TEXT_PRIMARY,
      lineHighlight: LIGHT_SURFACE_SUBTLE,
      selection: lightNavy(0.14),
      cursor: LIGHT_TEXT_PRIMARY,

      'editor.selectionBackground': lightNavy(0.14),
      'editor.selectionForeground': LIGHT_TEXT_PRIMARY,
      'editor.inactiveSelectionBackground': lightNavy(0.09),
      'editor.selectionHighlightBackground': lightNavy(0.1),
      'editor.selectionHighlightBorder': lightNavy(0.22),
      'editorCursor.foreground': LIGHT_TEXT_PRIMARY,

      'editor.wordHighlightBackground': lightNavy(0.07),
      'editor.wordHighlightStrongBackground': lightNavy(0.11),
    },
  },
};



