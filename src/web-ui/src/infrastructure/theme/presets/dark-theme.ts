

import { ThemeConfig } from '../types';
import {
  createAccentScale,
  createDarkNeutralBorder,
  createDarkNeutralElement,
  createDarkNeutralScrollbar,
  createGitColors,
  createSemanticColors,
  createSecondaryAccentScale,
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
      quaternary: '#262626',
      elevated: DARK_BACKGROUND_SECONDARY,
      workbench: DARK_BACKGROUND_PRIMARY,
      scene: DARK_BACKGROUND_SECONDARY,
      tooltip: rgbaFromHex(DARK_BACKGROUND_SECONDARY, 0.96),
    },

    text: {
      primary: DARK_TEXT_PRIMARY,
      secondary: '#b0b0b0',
      muted: '#858585',
      disabled: '#555555',
    },

    accent: createAccentScale({ base: DARK_ACCENT, hover: DARK_ACCENT_HOVER }),

    purple: createSecondaryAccentScale({ base: DARK_PURPLE, hover: DARK_PURPLE_HOVER }),

    semantic: createSemanticColors({
      success: DARK_SUCCESS,
      warning: DARK_WARNING,
      error: DARK_ERROR,
      info: '#a1a1aa',
      overrides: {
        infoBg: overlayWhite(0.08),
        infoBorder: overlayWhite(0.24),
      },
    }),

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
    },

    blur: {
      subtle: 'blur(4px) saturate(1.05)',
      base: 'blur(8px) saturate(1.1)',
    },

    radius: createStandardRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.6,
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

    windowControls: createWindowControls(DARK_ERROR),

    button: {



      primary: {
        default: {
          background: overlayWhite(0.16),
          color: '#f3f3f5',
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
          color: '#9a9a9a',
        },
        hover: {
          background: overlayWhite(0.1),
          color: DARK_BUTTON_TEXT,
          border: 'transparent',
        },
      },
    },
  },




  monaco: {
    base: 'vs-dark',
    inherit: true,
    rules: [],
    colors: {
      background: DARK_BACKGROUND_PRIMARY,
      foreground: DARK_TEXT_PRIMARY,
      lineHighlight: DARK_BACKGROUND_SECONDARY,
      selection: overlayWhite(0.12),
      cursor: DARK_BUTTON_TEXT,
    },
  },
};





