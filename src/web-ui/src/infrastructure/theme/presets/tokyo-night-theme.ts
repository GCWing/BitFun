

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

const TOKYO_BACKGROUND_PRIMARY = '#1a1b26';
const TOKYO_BACKGROUND_SECONDARY = '#16161e';
const TOKYO_TEXT_PRIMARY = '#c0caf5';
const TOKYO_TEXT_SECONDARY = '#a9b1d6';
const TOKYO_TEXT_MUTED = '#787c99';
const TOKYO_ACCENT = '#7aa2f7';
const TOKYO_ACCENT_HOVER = '#6183bb';
const TOKYO_PURPLE = '#bb9af7';
const TOKYO_PURPLE_HOVER = '#9d7cd8';
const TOKYO_SUCCESS = '#9ece6a';
const TOKYO_WARNING = '#e0af68';
const TOKYO_ERROR = '#f7768e';
const TOKYO_INFO = '#7dcfff';
const TOKYO_BORDER = '#363b54';
const TOKYO_SCROLLBAR = '#868bc4';
const TOKYO_GIT_ADDED = '#41a6b5';
const TOKYO_PRIMARY_BUTTON = '#3d59a1';

const tokyoAccent = (alpha: number | string) => rgbaFromHex(TOKYO_ACCENT, alpha);
const tokyoAccentHover = (alpha: number | string) => rgbaFromHex(TOKYO_ACCENT_HOVER, alpha);
const tokyoPurple = (alpha: number | string) => rgbaFromHex(TOKYO_PURPLE, alpha);
const tokyoPurpleHover = (alpha: number | string) => rgbaFromHex(TOKYO_PURPLE_HOVER, alpha);
const tokyoSuccess = (alpha: number | string) => rgbaFromHex(TOKYO_SUCCESS, alpha);
const tokyoWarning = (alpha: number | string) => rgbaFromHex(TOKYO_WARNING, alpha);
const tokyoError = (alpha: number | string) => rgbaFromHex(TOKYO_ERROR, alpha);
const tokyoInfo = (alpha: number | string) => rgbaFromHex(TOKYO_INFO, alpha);
const tokyoBorder = (alpha: number | string) => rgbaFromHex(TOKYO_BORDER, alpha);
const tokyoScrollbar = (alpha: number | string) => rgbaFromHex(TOKYO_SCROLLBAR, alpha);
const tokyoGitAdded = (alpha: number | string) => rgbaFromHex(TOKYO_GIT_ADDED, alpha);
const tokyoPrimaryButton = (alpha: number | string) => rgbaFromHex(TOKYO_PRIMARY_BUTTON, alpha);

/** Colors aligned with the Tokyo Night palette (Enkia / VS Code Tokyo Night). */
export const bitfunTokyoNightTheme: ThemeConfig = {
  id: 'bitfun-tokyo-night',
  name: 'Tokyo Night',
  type: 'dark',
  description:
    'Tokyo Night — deep indigo base, soft blue and magenta accents (palette from the Tokyo Night theme family)',
  author: 'BitFun Team',
  version: '1.0.0',

  colors: {
    background: {
      primary: TOKYO_BACKGROUND_PRIMARY,
      secondary: TOKYO_BACKGROUND_SECONDARY,
      tertiary: '#14141b',
      quaternary: '#1e202e',
      elevated: '#20222c',
      workbench: TOKYO_BACKGROUND_SECONDARY,
      scene: TOKYO_BACKGROUND_PRIMARY,
      tooltip: 'rgba(22, 22, 30, 0.94)',
    },

    text: {
      primary: TOKYO_TEXT_PRIMARY,
      secondary: TOKYO_TEXT_SECONDARY,
      muted: TOKYO_TEXT_MUTED,
      disabled: '#545c7e',
    },

    accent: {
      50: tokyoAccent(0.05),
      100: tokyoAccent(0.08),
      200: tokyoAccent(0.15),
      300: tokyoAccent(0.25),
      400: tokyoAccent(0.4),
      500: TOKYO_ACCENT,
      600: TOKYO_ACCENT_HOVER,
      700: tokyoAccentHover(0.85),
      800: tokyoAccentHover(0.95),
    },

    purple: {
      50: tokyoPurple(0.05),
      100: tokyoPurple(0.08),
      200: tokyoPurple(0.15),
      400: tokyoPurple(0.4),
      500: TOKYO_PURPLE,
      600: TOKYO_PURPLE_HOVER,
      800: tokyoPurpleHover(0.95),
    },

    semantic: {
      success: TOKYO_SUCCESS,
      successBg: tokyoSuccess(0.12),
      successBorder: tokyoSuccess(0.35),

      warning: TOKYO_WARNING,
      warningBg: tokyoWarning(0.12),
      warningBorder: tokyoWarning(0.35),

      error: TOKYO_ERROR,
      errorBg: tokyoError(0.12),
      errorBorder: tokyoError(0.35),

      info: TOKYO_INFO,
      infoBg: tokyoInfo(0.12),
      infoBorder: tokyoInfo(0.35),

    },

    border: {
      subtle: tokyoBorder(0.45),
      base: tokyoBorder(0.6),
      medium: tokyoBorder(0.72),
      strong: tokyoBorder(0.85),
      prominent: tokyoAccent(0.45),
    },

    element: {
      subtle: tokyoAccent(0.06),
      soft: tokyoAccent(0.08),
      base: tokyoAccent(0.11),
      medium: tokyoAccent(0.14),
      strong: tokyoAccent(0.18),
      elevated: tokyoAccent(0.22),
    },

    git: createGitColors({
      branch: rgbFromHex(TOKYO_ACCENT),
      branchBg: tokyoAccent(0.12),
      changes: rgbFromHex(TOKYO_WARNING),
      changesBg: tokyoWarning(0.12),
      added: rgbFromHex(TOKYO_GIT_ADDED),
      addedBg: tokyoGitAdded(0.12),
      deleted: rgbFromHex(TOKYO_ERROR),
      deletedBg: tokyoError(0.12),
      staged: rgbFromHex(TOKYO_SUCCESS),
      stagedBg: tokyoSuccess(0.12),
    }),

    scrollbar: {
      thumb: tokyoScrollbar(0.15),
      thumbHover: tokyoScrollbar(0.28),
    },
  },

  effects: {
    shadow: {
      xs: `0 1px 3px ${overlayBlack(0.55)}`,
      sm: `0 2px 6px ${overlayBlack(0.5)}`,
      base: `0 4px 12px ${overlayBlack(0.48)}`,
      lg: `0 8px 20px ${overlayBlack(0.45)}`,
      xl: `0 12px 28px ${overlayBlack(0.42)}`,
    },

    blur: {
      subtle: 'blur(4px) saturate(1.15)',
      base: 'blur(8px) saturate(1.2)',
    },

    radius: createCompactRadius(),

    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.5,
      hover: 0.88,
      focus: 0.96,
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
    windowControls: createWindowControls(TOKYO_ERROR),

    button: {

      primary: {
        default: {
          background: tokyoPrimaryButton(0.55),
          color: TOKYO_TEXT_PRIMARY,
          border: tokyoAccent(0.45),
          shadow: `0 0 14px ${tokyoPrimaryButton(0.35)}`,
        },
        hover: {
          background: tokyoPrimaryButton(0.72),
          color: STATIC_WHITE,
          border: tokyoAccent(0.55),
          shadow:
            `0 0 22px ${tokyoAccent(0.35)}, 0 4px 12px ${overlayBlack(0.35)}`,
          transform: 'translateY(-2px)',
        },
        active: {
          background: tokyoPrimaryButton(0.62),
          color: STATIC_WHITE,
          border: tokyoAccent(0.48),
          shadow: `0 0 18px ${tokyoAccent(0.28)}`,
          transform: 'translateY(-1px)',
        },
      },

      ghost: {
        default: {
          color: TOKYO_TEXT_MUTED,
        },
        hover: {
          background: tokyoAccent(0.1),
          color: TOKYO_TEXT_PRIMARY,
          border: tokyoAccent(0.35),
        },
      },
    },
  },

  monaco: {
    base: 'vs-dark',
    inherit: true,
    rules: [],
    colors: {
      background: TOKYO_BACKGROUND_PRIMARY,
      foreground: TOKYO_TEXT_SECONDARY,
      lineHighlight: '#1e202e',
      selection: 'rgba(81, 92, 126, 0.35)',
      cursor: TOKYO_TEXT_PRIMARY,
    },
  },
};
