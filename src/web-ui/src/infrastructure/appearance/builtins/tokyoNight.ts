

import { AppearancePalette } from './AppearancePalette';
import {
  createAccentScale,
  createCompactRadius,
  createGitColors,
  createSemanticColors,
  createSecondaryAccentScale,
  createStandardEasing,
  createStandardSpacing,
  overlayBlack,
  rgbFromHex,
  rgbaFromHex,
  STATIC_WHITE,
} from './paletteHelpers';

const TOKYO_CHROME = '#1a1b26';
const TOKYO_BACKGROUND_PRIMARY = '#24283b';
const TOKYO_BACKGROUND_SECONDARY = '#2f334d';
const TOKYO_TEXT_PRIMARY = '#c0caf5';
const TOKYO_TEXT_SECONDARY = '#a9b1d6';
const TOKYO_TEXT_MUTED = '#9aa5ce';
const TOKYO_ACCENT = '#7aa2f7';
const TOKYO_ACCENT_HOVER = '#6183bb';
const TOKYO_PURPLE = '#bb9af7';
const TOKYO_PURPLE_HOVER = '#9d7cd8';
const TOKYO_SCROLLBAR = '#868bc4';
const TOKYO_PRIMARY_BUTTON = '#3d59a1';

const tokyoAccent = (alpha: number | string) => rgbaFromHex(TOKYO_ACCENT, alpha);
const tokyoBorder = (alpha: number | string) => rgbaFromHex(TOKYO_TEXT_SECONDARY, alpha);
const tokyoScrollbar = (alpha: number | string) => rgbaFromHex(TOKYO_SCROLLBAR, alpha);
const tokyoPrimaryButton = (alpha: number | string) => rgbaFromHex(TOKYO_PRIMARY_BUTTON, alpha);

/** Colors aligned with the Tokyo Night palette (Enkia / VS Code Tokyo Night). */
export const openBitFunTokyoNightPalette: AppearancePalette = {
  id: 'openbitfun-tokyo-night',
  name: 'Tokyo Night',
  type: 'dark',
  description:
    'Tokyo Night - deep indigo base with soft blue and magenta accents',
  author: 'OpenBitFun Team',
  version: '1.1.0',

  colors: {
    background: {
      // The night base frames storm-indigo content and brighter blue-violet panels.
      primary: TOKYO_BACKGROUND_PRIMARY,
      secondary: TOKYO_BACKGROUND_SECONDARY,
      tertiary: TOKYO_CHROME,
      elevated: TOKYO_BACKGROUND_SECONDARY,
      workbench: TOKYO_CHROME,
      scene: TOKYO_BACKGROUND_PRIMARY,
      chrome: TOKYO_CHROME,
    },

    text: {
      primary: TOKYO_TEXT_PRIMARY,
      secondary: TOKYO_TEXT_SECONDARY,
      muted: TOKYO_TEXT_MUTED,
      disabled: '#545c7e',
    },

    accent: createAccentScale({
      base: TOKYO_ACCENT,
      hover: TOKYO_ACCENT_HOVER,
      alpha: { 50: 0.05, 700: 0.85 },
    }),

    purple: createSecondaryAccentScale({
      base: TOKYO_PURPLE,
      hover: TOKYO_PURPLE_HOVER,
    }),

    semantic: createSemanticColors('dark'),

    border: {
      subtle: tokyoBorder(0.18),
      base: tokyoBorder(0.26),
      medium: tokyoBorder(0.34),
      strong: tokyoBorder(0.42),
      prominent: tokyoAccent(0.45),
    },

    element: {
      subtle: tokyoAccent(0.06),
      soft: tokyoAccent(0.08),
      base: tokyoAccent(0.11),
      medium: tokyoAccent(0.14),
      strong: tokyoAccent(0.18),
    },

    git: createGitColors('dark', {
      branch: rgbFromHex(TOKYO_ACCENT),
      branchBg: tokyoAccent(0.12),
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
      base: '0.2s',
      slow: '0.4s',
    },

    easing: createStandardEasing(),
  },

  components: {
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
      lineHighlight: TOKYO_BACKGROUND_SECONDARY,
      selection: 'rgba(81, 92, 126, 0.35)',
      cursor: TOKYO_TEXT_PRIMARY,
    },
  },
};
