

import { ThemeConfig } from '../types';
import {
  createAccentScale,
  createDarkNeutralBorder,
  createDarkNeutralElement,
  createDarkNeutralScrollbar,
  createGitColors,
  createSemanticColors,
  createSecondaryAccentScale,
  createSlateRadius,
  createStandardEasing,
  createStandardSpacing,
  createStandardTypography,
  createWindowControls,
  overlayBlack,
  overlayWhite,
  rgbFromHex,
  rgbaFromHex,
  STATIC_WHITE,
} from './shared';

const SLATE_BACKGROUND_PRIMARY = '#1c1c1f';
const SLATE_BACKGROUND_SECONDARY = '#262626';
const SLATE_TEXT_PRIMARY = '#e8e8e8';
const SLATE_TEXT_MUTED = '#a8b0bd';
const SLATE_BUTTON_TEXT = '#e0e3e8';
const SLATE_ACCENT_SOFT = '#e8e8e8';
const SLATE_ACCENT = '#94a3b8';
const SLATE_ACCENT_HOVER = '#64748b';
const SLATE_PURPLE = '#b8c4ff';
const SLATE_PURPLE_HOVER = '#9dacf5';
const SLATE_SUCCESS = '#7fb899';
const SLATE_WARNING = '#f59e0b';
const SLATE_ERROR = '#c9878d';

const slateSuccess = (alpha: number | string) => rgbaFromHex(SLATE_SUCCESS, alpha);
const slateWarning = (alpha: number | string) => rgbaFromHex(SLATE_WARNING, alpha);
const slateError = (alpha: number | string) => rgbaFromHex(SLATE_ERROR, alpha);

export const bitfunSlateTheme: ThemeConfig = {

  id: 'bitfun-slate',
  name: 'Slate',
  type: 'dark',
  description: 'Slate gray geometric theme - Deep immersion, high contrast grayscale aesthetics',
  author: 'BitFun Team',
  version: '1.3.0',

  layout: {
    sceneViewportBorder: false,
  },

  colors: {
    background: {
      primary: SLATE_BACKGROUND_PRIMARY,
      secondary: SLATE_BACKGROUND_SECONDARY,
      tertiary: SLATE_BACKGROUND_PRIMARY,
      quaternary: '#313335',
      elevated: SLATE_BACKGROUND_SECONDARY,
      workbench: SLATE_BACKGROUND_PRIMARY,
      scene: SLATE_BACKGROUND_SECONDARY,
      tooltip: rgbaFromHex(SLATE_BACKGROUND_SECONDARY, 0.96),
    },

    text: {
      primary: SLATE_TEXT_PRIMARY,
      secondary: '#c8ccd2',
      muted: '#a1a1aa',
      disabled: '#6a6a6a',
    },


    // Cool gray accent — neutral chrome for slate surfaces (links, focus, nav tints).
    accent: createAccentScale({
      base: SLATE_ACCENT,
      hover: SLATE_ACCENT_HOVER,
      alpha: { 700: 0.85 },
      stops: {
        50: rgbaFromHex(SLATE_ACCENT_SOFT, 0.05),
        100: rgbaFromHex(SLATE_ACCENT_SOFT, 0.09),
        200: 'rgba(203, 213, 225, 0.14)',
        300: 'rgba(203, 213, 225, 0.24)',
        400: 'rgba(148, 163, 184, 0.45)',
        800: 'rgba(71, 85, 105, 0.92)',
      },
    }),


    purple: createSecondaryAccentScale({ base: SLATE_PURPLE, hover: SLATE_PURPLE_HOVER }),

    semantic: createSemanticColors({
      success: SLATE_SUCCESS,
      warning: SLATE_WARNING,
      error: SLATE_ERROR,
      info: SLATE_TEXT_MUTED,
      overrides: {
        infoBg: overlayWhite(0.07),
        infoBorder: overlayWhite(0.2),
      },
    }),

    border: createDarkNeutralBorder(),

    element: createDarkNeutralElement(),

    git: createGitColors({
      branch: SLATE_ACCENT,
      branchBg: overlayWhite(0.06),
      changes: rgbFromHex(SLATE_WARNING),
      changesBg: slateWarning(0.1),
      added: rgbFromHex(SLATE_SUCCESS),
      addedBg: slateSuccess(0.1),
      deleted: rgbFromHex(SLATE_ERROR),
      deletedBg: slateError(0.1),
    }),

    scrollbar: createDarkNeutralScrollbar(),
  },


  effects: {
    shadow: {
      xs: `0 1px 2px ${overlayBlack(0.85)}`,
      sm: `0 2px 4px ${overlayBlack(0.8)}`,
      base: `0 4px 8px ${overlayBlack(0.75)}`,
      lg: `0 8px 16px ${overlayBlack(0.7)}`,
      xl: `0 12px 24px ${overlayBlack(0.85)}`,
    },

    blur: {
      subtle: 'blur(4px) saturate(1.05) brightness(0.98)',
      base: 'blur(8px) saturate(1.08) brightness(0.98)',
    },

    radius: createSlateRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.5,
      hover: 0.75,
      focus: 0.85,
    },
  },


  motion: {
    duration: {
      instant: '0.08s',
      fast: '0.12s',
      base: '0.25s',
      slow: '0.5s',
    },

    easing: createStandardEasing(),
  },


  typography: createStandardTypography(),


  components: {

    windowControls: createWindowControls(SLATE_ERROR),

    button: {



      primary: {
        default: {
          background: overlayWhite(0.14),
          color: SLATE_TEXT_PRIMARY,
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: overlayWhite(0.2),
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: overlayWhite(0.17),
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },


      ghost: {
        default: {
          color: SLATE_TEXT_MUTED,
        },
        hover: {
          background: overlayWhite(0.08),
          color: SLATE_BUTTON_TEXT,
          border: 'transparent',
        },
      },
    },
  },


  monaco: {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '9ca2a9', fontStyle: 'italic' },
      { token: 'keyword', foreground: 'a8b4c4' },
      { token: 'string', foreground: '8fc8a9' },
      { token: 'number', foreground: 'b5c4fc' },
      { token: 'type', foreground: '9ca6b8' },
      { token: 'class', foreground: '9ca6b8' },
      { token: 'function', foreground: 'c5cad3' },
      { token: 'variable', foreground: 'c4c8ce' },
      { token: 'constant', foreground: 'b5c4fc' },
      { token: 'operator', foreground: 'a8b4c4' },
      { token: 'tag', foreground: '9ca6b8' },
      { token: 'attribute.name', foreground: 'c4c8ce' },
      { token: 'attribute.value', foreground: '8fc8a9' },
    ],
    colors: {
      background: SLATE_BACKGROUND_PRIMARY,
      foreground: SLATE_TEXT_PRIMARY,
      lineHighlight: SLATE_BACKGROUND_SECONDARY,
      selection: overlayWhite(0.12),
      cursor: '#aeb6c3',
    },
  },
};
