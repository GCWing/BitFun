

import { AppearancePalette } from './AppearancePalette';
import {
  createAccentScale,
  createGitColors,
  createSemanticColors,
  createSecondaryAccentScale,
  createStandardEasing,
  createStandardRadius,
  createStandardSpacing,
  rgbFromHex,
  rgbaFromHex,
  STATIC_BLACK,
  STATIC_WHITE,
} from './paletteHelpers';
import {
  getDesignSystemThemeNumber,
  getDesignSystemThemeString,
} from './designSystemThemeValues';

const LIGHT_NAVY = getDesignSystemThemeString('light', 'color.action.primary.background');
const LIGHT_TEXT_PRIMARY = getDesignSystemThemeString('light', 'color.content.primary');
const LIGHT_TEXT_SECONDARY = getDesignSystemThemeString('light', 'color.content.secondary');
const LIGHT_TEXT_MUTED = getDesignSystemThemeString('light', 'color.content.muted');
const LIGHT_TEXT_DISABLED = getDesignSystemThemeString('light', 'color.content.disabled');
const LIGHT_NAVY_HOVER = getDesignSystemThemeString('light', 'color.action.primary.hover');
const LIGHT_PURPLE = '#7c6b99';
const LIGHT_PURPLE_HOVER = '#655680';
const LIGHT_SUCCESS = getDesignSystemThemeString('light', 'color.status.success.content');
const LIGHT_SUCCESS_BG = getDesignSystemThemeString('light', 'color.status.success.surface');
const LIGHT_WARNING = getDesignSystemThemeString('light', 'color.status.warning.content');
const LIGHT_ERROR = getDesignSystemThemeString('light', 'color.status.danger.content');
const LIGHT_ERROR_BG = getDesignSystemThemeString('light', 'color.status.danger.surface');
const LIGHT_BACKGROUND_PRIMARY = getDesignSystemThemeString('light', 'color.surface.canvas');
const LIGHT_SURFACE_CHROME = getDesignSystemThemeString('light', 'color.surface.chrome');
const LIGHT_SURFACE_SUBTLE = getDesignSystemThemeString('light', 'color.surface.subtle');
const LIGHT_SURFACE_TERTIARY = getDesignSystemThemeString('light', 'color.surface.tertiary');
const LIGHT_SURFACE_SOFT = getDesignSystemThemeString('light', 'color.action.quiet.hover');
const LIGHT_BORDER_BASE = getDesignSystemThemeString('light', 'color.border.default');

const lightNavy = (alpha: number | string) => rgbaFromHex(LIGHT_NAVY, alpha);
const lightNavyHover = (alpha: number | string) => rgbaFromHex(LIGHT_NAVY_HOVER, alpha);

export const bitfunLightPalette: AppearancePalette = {

  id: 'bitfun-light',
  name: 'Light',
  type: 'light',
  description: 'Light appearance - Crisp white surfaces, soft neutral grays, deep navy actions',
  author: 'BitFun Team',
  version: '2.5.0',

  layout: {
    sceneViewportBorder: false,
  },


  colors: {
    background: {
      primary: LIGHT_BACKGROUND_PRIMARY,
      secondary: STATIC_WHITE,
      tertiary: LIGHT_SURFACE_TERTIARY,
      elevated: STATIC_WHITE,
      workbench: LIGHT_SURFACE_SOFT,
      scene: STATIC_WHITE,
      chrome: LIGHT_SURFACE_CHROME,
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
        successBorder: getDesignSystemThemeString('light', 'color.status.success.border'),
        warningBg: getDesignSystemThemeString('light', 'color.status.warning.surface'),
        warningBorder: getDesignSystemThemeString('light', 'color.status.warning.border'),
        errorBg: LIGHT_ERROR_BG,
        errorBorder: getDesignSystemThemeString('light', 'color.status.danger.border'),
        infoBg: getDesignSystemThemeString('light', 'color.status.info.surface'),
        infoBorder: getDesignSystemThemeString('light', 'color.status.info.border'),
      },
    }),


    border: {
      subtle: getDesignSystemThemeString('light', 'color.border.subtle'),
      base: LIGHT_BORDER_BASE,
      medium: lightNavy(0.24),
      strong: getDesignSystemThemeString('light', 'color.border.strong'),
      prominent: lightNavy(0.48),
    },


    element: {
      subtle: LIGHT_SURFACE_SUBTLE,
      soft: LIGHT_SURFACE_SOFT,
      base: getDesignSystemThemeString('light', 'color.action.neutral.surface'),
      medium: getDesignSystemThemeString('light', 'color.action.neutral.surfaceHover'),
      strong: getDesignSystemThemeString('light', 'color.action.neutral.surfacePressed'),
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
      xs: getDesignSystemThemeString('light', 'shadow.xs'),
      sm: getDesignSystemThemeString('light', 'shadow.sm'),
      base: getDesignSystemThemeString('light', 'shadow.base'),
      lg: getDesignSystemThemeString('light', 'shadow.lg'),
      xl: getDesignSystemThemeString('light', 'shadow.xl'),
    },


    blur: {
      subtle: getDesignSystemThemeString('light', 'effect.blur.subtle'),
      base: getDesignSystemThemeString('light', 'effect.blur.base'),
    },

    radius: createStandardRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: getDesignSystemThemeNumber('light', 'opacity.disabled'),
      hover: getDesignSystemThemeNumber('light', 'opacity.hover'),
      focus: getDesignSystemThemeNumber('light', 'opacity.focus'),
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



  components: {
    button: {



      primary: {
        default: {
          background: getDesignSystemThemeString('light', 'color.action.primary.background'),
          color: getDesignSystemThemeString('light', 'color.action.primary.content'),
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: getDesignSystemThemeString('light', 'color.action.primary.hover'),
          color: getDesignSystemThemeString('light', 'color.action.primary.content'),
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: getDesignSystemThemeString('light', 'color.action.primary.pressed'),
          color: getDesignSystemThemeString('light', 'color.action.primary.content'),
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


