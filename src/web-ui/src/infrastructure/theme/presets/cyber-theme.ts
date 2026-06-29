

import { ThemeConfig } from '../types';
import {
  createCompactRadius,
  createExpressiveTypography,
  createGitColors,
  createStandardEasing,
  createStandardSpacing,
  createWindowControls,
  overlayBlack,
  rgbFromHex,
  rgbaFromHex,
  STATIC_WHITE,
} from './shared';

const CYBER_BACKGROUND = '#101010';
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

const cyberAccent = (alpha: number | string) => rgbaFromHex(CYBER_ACCENT, alpha);
const cyberAccentHover = (alpha: number | string) => rgbaFromHex(CYBER_ACCENT_HOVER, alpha);
const cyberPurple = (alpha: number | string) => rgbaFromHex(CYBER_PURPLE, alpha);
const cyberPurpleHover = (alpha: number | string) => rgbaFromHex(CYBER_PURPLE_HOVER, alpha);
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
      secondary: '#151515',
      tertiary: '#1a1a1a',
      quaternary: '#1f1f1f',
      elevated: '#0d0d0d',
      workbench: CYBER_BACKGROUND,
      scene: '#141414',
      tooltip: 'rgba(16, 16, 16, 0.95)',
    },

    text: {
      primary: CYBER_TEXT_PRIMARY,
      secondary: CYBER_TEXT_SECONDARY,
      muted: CYBER_TEXT_MUTED,
      disabled: '#4a5a66',
    },

    accent: {
      50: cyberAccent(0.05),
      100: cyberAccent(0.1),
      200: cyberAccent(0.18),
      300: cyberAccent(0.3),
      400: cyberAccent(0.45),
      500: CYBER_ACCENT,
      600: CYBER_ACCENT_HOVER,
      700: cyberAccentHover(0.85),
      800: cyberAccentHover(0.95),
    },

    purple: {
      50: cyberPurple(0.05),
      100: cyberPurple(0.1),
      200: cyberPurple(0.18),
      300: cyberPurple(0.3),
      400: cyberPurple(0.45),
      500: CYBER_PURPLE,
      600: CYBER_PURPLE_HOVER,
      700: cyberPurpleHover(0.85),
      800: cyberPurpleHover(0.95),
    },

    semantic: {
      success: CYBER_SUCCESS,
      successBg: cyberSuccess(0.12),
      successBorder: cyberSuccess(0.35),

      warning: CYBER_WARNING,
      warningBg: cyberWarning(0.12),
      warningBorder: cyberWarning(0.35),

      error: CYBER_ERROR,
      errorBg: cyberError(0.12),
      errorBorder: cyberError(0.35),

      info: CYBER_ACCENT,
      infoBg: cyberAccent(0.12),
      infoBorder: cyberAccent(0.35),


      highlight: '#ffdd44',
      highlightBg: 'rgba(255, 221, 68, 0.15)',
    },

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
      '2xl': '0 16px 36px rgba(0, 0, 0, 0.65)',
    },

    glow: {

      blue: `0 0 12px ${cyberAccent(0.4)}, 0 0 24px ${cyberAccent(0.25)}, 0 0 36px ${cyberAccent(0.15)}, 0 4px 16px ${overlayBlack(0.3)}`,

      purple: `0 0 12px ${cyberPurple(0.4)}, 0 0 24px ${cyberPurple(0.25)}, 0 0 36px ${cyberPurple(0.15)}, 0 4px 16px ${overlayBlack(0.3)}`,

      mixed: `0 0 16px ${cyberAccent(0.35)}, 0 0 28px ${cyberPurple(0.25)}, 0 0 40px ${cyberAccent(0.12)}, 0 4px 20px ${overlayBlack(0.35)}`,
    },

    blur: {
      subtle: 'blur(4px) saturate(1.2)',
      base: 'blur(8px) saturate(1.3)',
      medium: 'blur(12px) saturate(1.4)',
      strong: 'blur(16px) saturate(1.5) brightness(1.15)',
      intense: 'blur(20px) saturate(1.6) brightness(1.2)',
    },

    radius: createCompactRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.5,
      hover: 0.85,
      focus: 0.95,
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

    easing: createStandardEasing('cubic-bezier(0.25, 0.46, 0.45, 0.94)'),
  },


  typography: createExpressiveTypography(),


  components: {

    windowControls: createWindowControls({
      standard: {
        dot: cyberAccent(0.5),
        dotShadow: `0 0 6px ${cyberAccent(0.35)}`,
        hoverBg: cyberAccent(0.15),
        hoverColor: CYBER_ACCENT,
        hoverBorder: cyberAccent(0.3),
        hoverShadow: `0 0 12px ${cyberAccent(0.3)}, 0 2px 8px ${cyberAccent(0.2)}, inset 0 1px 0 ${cyberAccent(0.2)}`,
      },
      close: {
        dot: cyberError(0.5),
        dotShadow: `0 0 6px ${cyberError(0.35)}`,
        hoverBg: cyberError(0.15),
        hoverColor: CYBER_ERROR,
        hoverBorder: cyberError(0.3),
        hoverShadow: `0 0 12px ${cyberError(0.3)}, 0 2px 8px ${cyberError(0.2)}, inset 0 1px 0 ${cyberError(0.2)}`,
      },
      common: {
        defaultColor: rgbaFromHex(CYBER_TEXT_PRIMARY, 0.9),
        defaultDot: cyberAccent(0.2),
        disabledDot: cyberAccent(0.1),
        flowGradient: `linear-gradient(90deg, transparent, ${cyberAccent(0.08)}, ${cyberAccent(0.12)}, ${cyberAccent(0.08)}, transparent)`,
      },
    }),

    button: {

      default: {
        background: cyberAccent(0.08),
        color: CYBER_TEXT_MUTED,
        border: cyberAccent(0.15),
        shadow: `0 0 8px ${cyberAccent(0.1)}`,
      },
      hover: {
        background: cyberAccent(0.14),
        color: CYBER_TEXT_SECONDARY,
        border: cyberAccent(0.3),
        shadow: `0 0 16px ${cyberAccent(0.2)}, 0 2px 8px ${overlayBlack(0.3)}`,
        transform: 'translateY(-1px)',
      },
      active: {
        background: cyberAccent(0.12),
        color: CYBER_TEXT_SECONDARY,
        border: cyberAccent(0.35),
        shadow: `0 0 12px ${cyberAccent(0.15)}`,
        transform: 'translateY(0)',
      },


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
          background: 'transparent',
          color: CYBER_TEXT_MUTED,
          border: cyberAccent(0.2),
          shadow: 'none',
        },
        hover: {
          background: cyberAccent(0.1),
          color: CYBER_TEXT_SECONDARY,
          border: cyberAccent(0.35),
          shadow: `0 0 12px ${cyberAccent(0.15)}`,
          transform: 'translateY(-1px)',
        },
        active: {
          background: cyberAccent(0.08),
          color: CYBER_TEXT_SECONDARY,
          border: cyberAccent(0.3),
          shadow: `0 0 8px ${cyberAccent(0.1)}`,
          transform: 'translateY(0)',
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
      lineHighlight: '#151515',
      selection: '#1a4d66',
      cursor: CYBER_ACCENT,
    },
  },
};

