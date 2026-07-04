

import { ThemeConfig } from '../types';
import {
  createAccentScale,
  createCompactRadius,
  createExpressiveTypography,
  createGitColors,
  createSemanticColors,
  createSecondaryAccentScale,
  createStandardEasing,
  createStandardSpacing,
  createWindowControls,
  overlayBlack,
  rgbFromHex,
  rgbaFromHex,
  STATIC_WHITE,
} from './shared';

const CYBER_BACKGROUND = '#0e0e10';
const CYBER_TEXT_PRIMARY = '#e0f2ff';
const CYBER_TEXT_SECONDARY = '#c7e7ff';
const CYBER_TEXT_MUTED = '#7fadcc';
const CYBER_ACCENT = '#00e6ff';
const CYBER_ACCENT_HOVER = '#00ccff';
const CYBER_PURPLE = '#8a2be2';
const CYBER_PURPLE_HOVER = '#7928ca';
const CYBER_SUCCESS = '#00ff9f';
const CYBER_WARNING = '#ffcc00';
const CYBER_ERROR = '#ff0055';
const CYBER_SURFACE_SECONDARY = '#1c1c1f';

const cyberAccent = (alpha: number | string) => rgbaFromHex(CYBER_ACCENT, alpha);
const cyberSuccess = (alpha: number | string) => rgbaFromHex(CYBER_SUCCESS, alpha);
const cyberWarning = (alpha: number | string) => rgbaFromHex(CYBER_WARNING, alpha);
const cyberError = (alpha: number | string) => rgbaFromHex(CYBER_ERROR, alpha);

export const bitfunCyberTheme: ThemeConfig = {

  id: 'bitfun-cyber',
  name: 'Cyber',
  type: 'dark',
  description: 'Tech-style theme - Deep black hole, neon future, ultimate tech aesthetics',
  author: 'BitFun Team',
  version: '1.0.0',


  colors: {
    background: {
      primary: CYBER_BACKGROUND,
      secondary: CYBER_SURFACE_SECONDARY,
      tertiary: CYBER_SURFACE_SECONDARY,
      quaternary: CYBER_SURFACE_SECONDARY,
      elevated: CYBER_BACKGROUND,
      workbench: CYBER_BACKGROUND,
      scene: CYBER_SURFACE_SECONDARY,
      tooltip: rgbaFromHex(CYBER_BACKGROUND, 0.95),
    },

    text: {
      primary: CYBER_TEXT_PRIMARY,
      secondary: CYBER_TEXT_SECONDARY,
      muted: CYBER_TEXT_MUTED,
      disabled: '#475569',
    },

    accent: createAccentScale({
      base: CYBER_ACCENT,
      hover: CYBER_ACCENT_HOVER,
      alpha: { 50: 0.05, 100: 0.1, 200: 0.18, 300: 0.3, 400: 0.45, 700: 0.85, 800: 0.95 },
    }),

    purple: createSecondaryAccentScale({
      base: CYBER_PURPLE,
      hover: CYBER_PURPLE_HOVER,
      alpha: { 50: 0.05, 100: 0.1, 200: 0.18, 400: 0.45, 800: 0.95 },
    }),

    semantic: createSemanticColors({
      success: CYBER_SUCCESS,
      warning: CYBER_WARNING,
      error: CYBER_ERROR,
      info: CYBER_ACCENT,
      bgAlpha: 0.12,
      borderAlpha: 0.35,
    }),

    border: {
      subtle: cyberAccent(0.14),
      base: cyberAccent(0.2),
      medium: cyberAccent(0.28),
      strong: cyberAccent(0.36),
      prominent: cyberAccent(0.5),
    },

    element: {
      subtle: cyberAccent(0.06),
      soft: cyberAccent(0.09),
      base: cyberAccent(0.13),
      medium: cyberAccent(0.17),
      strong: cyberAccent(0.22),
      elevated: cyberAccent(0.27),
    },

    git: createGitColors({
      branch: rgbFromHex(CYBER_ACCENT),
      branchBg: cyberAccent(0.12),
      changes: rgbFromHex(CYBER_WARNING),
      changesBg: cyberWarning(0.12),
      added: rgbFromHex(CYBER_SUCCESS),
      addedBg: cyberSuccess(0.12),
      deleted: rgbFromHex(CYBER_ERROR),
      deletedBg: cyberError(0.12),
    }),
  },


  effects: {
    shadow: {
      xs: '0 1px 3px rgba(0, 0, 0, 0.9)',
      sm: '0 2px 6px rgba(0, 0, 0, 0.85)',
      base: '0 4px 12px rgba(0, 0, 0, 0.8)',
      lg: '0 8px 20px rgba(0, 0, 0, 0.75)',
      xl: '0 12px 28px rgba(0, 0, 0, 0.7)',
    },

    blur: {
      subtle: 'blur(4px) saturate(1.2)',
      base: 'blur(8px) saturate(1.3)',
    },

    radius: createCompactRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.5,
      hover: 0.85,
      focus: 0.95,
    },
  },


  motion: {
    duration: {
      instant: '0.08s',
      fast: '0.12s',
      base: '0.25s',
      slow: '0.5s',
    },

    easing: createStandardEasing('cubic-bezier(0.25, 0.46, 0.45, 0.94)'),
  },


  typography: createExpressiveTypography(),


  components: {

    windowControls: createWindowControls(CYBER_ERROR),

    button: {



      primary: {
        default: {
          background: cyberAccent(0.18),
          color: CYBER_TEXT_PRIMARY,
          border: cyberAccent(0.4),
          shadow: `0 0 16px ${cyberAccent(0.25)}`,
        },
        hover: {
          background: cyberAccent(0.25),
          color: STATIC_WHITE,
          border: cyberAccent(0.6),
          shadow: `0 0 24px ${cyberAccent(0.4)}, 0 0 36px ${cyberAccent(0.2)}, 0 4px 12px ${overlayBlack(0.3)}`,
          transform: 'translateY(-2px)',
        },
        active: {
          background: cyberAccent(0.22),
          color: STATIC_WHITE,
          border: cyberAccent(0.5),
          shadow: `0 0 20px ${cyberAccent(0.3)}`,
          transform: 'translateY(-1px)',
        },
      },


      ghost: {
        default: {
          color: CYBER_TEXT_MUTED,
        },
        hover: {
          background: cyberAccent(0.1),
          color: CYBER_TEXT_SECONDARY,
          border: cyberAccent(0.35),
        },
      },
    },
  },


  monaco: {
    base: 'vs-dark',
    inherit: true,
    rules: [],
    colors: {
      background: CYBER_BACKGROUND,
      foreground: CYBER_TEXT_SECONDARY,
      lineHighlight: CYBER_SURFACE_SECONDARY,
      selection: '#1a4d66',
      cursor: CYBER_ACCENT,
    },
  },
};

