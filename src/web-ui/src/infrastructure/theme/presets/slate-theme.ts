

import { ThemeConfig } from '../types';
import {
  createDarkNeutralBorder,
  createDarkNeutralElement,
  createDarkNeutralScrollbar,
  createGitColors,
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

const SLATE_BACKGROUND_PRIMARY = '#14161a';
const SLATE_BACKGROUND_SECONDARY = '#22262c';
const SLATE_TEXT_PRIMARY = '#eef0f3';
const SLATE_TEXT_MUTED = '#a8b0bd';
const SLATE_BUTTON_TEXT = '#dce0e6';
const SLATE_ACCENT_SOFT = '#e2e8f0';
const SLATE_ACCENT = '#94a3b8';
const SLATE_ACCENT_HOVER = '#64748b';
const SLATE_PURPLE = '#b8c4ff';
const SLATE_PURPLE_HOVER = '#9dacf5';
const SLATE_SUCCESS = '#7fb899';
const SLATE_WARNING = '#f59e0b';
const SLATE_ERROR = '#c9878d';

const slateAccent = (alpha: number | string) => rgbaFromHex(SLATE_ACCENT, alpha);
const slateAccentHover = (alpha: number | string) => rgbaFromHex(SLATE_ACCENT_HOVER, alpha);
const slatePurple = (alpha: number | string) => rgbaFromHex(SLATE_PURPLE, alpha);
const slatePurpleHover = (alpha: number | string) => rgbaFromHex(SLATE_PURPLE_HOVER, alpha);
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
      quaternary: '#2c3038',
      elevated: SLATE_BACKGROUND_SECONDARY,
      workbench: SLATE_BACKGROUND_PRIMARY,
      scene: SLATE_BACKGROUND_SECONDARY,
      tooltip: 'rgba(34, 38, 44, 0.96)',
    },

    text: {
      primary: SLATE_TEXT_PRIMARY,
      secondary: '#c8ccd2',
      muted: '#9ea4ab',
      disabled: '#65696f',
    },


    // Cool gray accent — neutral chrome for slate surfaces (links, focus, nav tints).
    accent: {
      50: rgbaFromHex(SLATE_ACCENT_SOFT, 0.05),
      100: rgbaFromHex(SLATE_ACCENT_SOFT, 0.09),
      200: 'rgba(203, 213, 225, 0.14)',
      300: 'rgba(203, 213, 225, 0.24)',
      400: 'rgba(148, 163, 184, 0.45)',
      500: SLATE_ACCENT,
      600: SLATE_ACCENT_HOVER,
      700: slateAccentHover(0.85),
      800: 'rgba(71, 85, 105, 0.92)',
    },


    purple: {
      50: slatePurple(0.04),
      100: slatePurple(0.08),
      200: slatePurple(0.15),
      300: slatePurple(0.25),
      400: slatePurple(0.4),
      500: SLATE_PURPLE,
      600: SLATE_PURPLE_HOVER,
      700: slatePurpleHover(0.8),
      800: slatePurpleHover(0.9),
    },

    semantic: {
      success: SLATE_SUCCESS,
      successBg: slateSuccess(0.1),
      successBorder: slateSuccess(0.3),

      warning: SLATE_WARNING,
      warningBg: slateWarning(0.1),
      warningBorder: slateWarning(0.3),

      error: SLATE_ERROR,
      errorBg: slateError(0.1),
      errorBorder: slateError(0.3),

      info: SLATE_TEXT_MUTED,
      infoBg: overlayWhite(0.07),
      infoBorder: overlayWhite(0.2),


      highlight: '#c8cdd4',
      highlightBg: overlayWhite(0.1),
    },

    border: createDarkNeutralBorder(),

    element: createDarkNeutralElement(),

    git: createGitColors({
      branch: '#9ca6b8',
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
      '2xl': `0 16px 32px ${overlayBlack(0.9)}`,
    },

    glow: {
      blue: `0 12px 32px ${slateAccent(0.14)}, 0 6px 16px ${slateAccent(0.1)}, 0 3px 8px ${overlayBlack(0.2)}`,
      purple: `0 12px 32px ${slatePurple(0.2)}, 0 6px 16px ${slatePurple(0.12)}, 0 3px 8px ${overlayBlack(0.2)}`,
      mixed: `0 12px 32px ${overlayWhite(0.05)}, 0 6px 16px ${slatePurple(0.1)}, 0 3px 8px ${overlayBlack(0.18)}`,
    },

    blur: {
      subtle: 'blur(4px) saturate(1.05) brightness(0.98)',
      base: 'blur(8px) saturate(1.08) brightness(0.98)',
      medium: 'blur(12px) saturate(1.12) brightness(0.97)',
      strong: 'blur(16px) saturate(1.15) brightness(0.97)',
      intense: 'blur(20px) saturate(1.18) brightness(0.96)',
    },

    radius: createSlateRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.5,
      hover: 0.75,
      focus: 0.85,
      overlay: 0.5,
    },
  },


  motion: {
    duration: {
      instant: '0.08s',
      fast: '0.12s',
      base: '0.25s',
      slow: '0.5s',
      lazy: '0.8s',
    },

    easing: createStandardEasing(),
  },


  typography: createStandardTypography(),


  components: {

    windowControls: createWindowControls({
      standard: {
        dot: 'rgba(203, 213, 225, 0.42)',
        dotShadow: `0 0 4px ${overlayBlack(0.35)}`,
        hoverBg: overlayWhite(0.09),
        hoverColor: SLATE_ACCENT_SOFT,
        hoverBorder: overlayWhite(0.14),
        hoverShadow: `0 2px 8px ${overlayBlack(0.22)}, inset 0 1px 0 ${overlayWhite(0.06)}`,
      },
      close: {
        dot: slateError(0.5),
        dotShadow: `0 0 4px ${slateError(0.25)}`,
        hoverBg: slateError(0.15),
        hoverColor: SLATE_ERROR,
        hoverBorder: slateError(0.25),
        hoverShadow: `0 2px 8px ${slateError(0.18)}, inset 0 1px 0 ${overlayWhite(0.08)}`,
      },
      common: {
        defaultColor: 'rgba(232, 234, 236, 0.92)',
        defaultDot: 'rgba(198, 202, 208, 0.48)',
        disabledDot: 'rgba(168, 171, 176, 0.2)',
        flowGradient: `linear-gradient(90deg, transparent, ${overlayWhite(0.05)}, ${overlayWhite(0.08)}, ${overlayWhite(0.05)}, transparent)`,
      },
    }),

    button: {

      default: {
        background: overlayWhite(0.08),
        color: SLATE_TEXT_MUTED,
        border: 'transparent',
        shadow: 'none',
      },
      hover: {
        background: overlayWhite(0.12),
        color: SLATE_BUTTON_TEXT,
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      active: {
        background: overlayWhite(0.1),
        color: SLATE_BUTTON_TEXT,
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },


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
          background: 'transparent',
          color: SLATE_TEXT_MUTED,
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: overlayWhite(0.08),
          color: SLATE_BUTTON_TEXT,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: overlayWhite(0.06),
          color: SLATE_BUTTON_TEXT,
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
      background: '#1a1c1e',
      foreground: SLATE_TEXT_PRIMARY,
      lineHighlight: '#22252a',
      selection: overlayWhite(0.12),
      cursor: '#aeb6c3',
    },
  },
};
