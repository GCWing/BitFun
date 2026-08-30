import type { AppearancePalette } from './AppearancePalette';
import { bitfunDarkPalette } from './dark';
import { bitfunLightPalette } from './light';
import {
  createAccentScale,
  createSecondaryAccentScale,
  STATIC_BLACK,
  STATIC_WHITE,
} from './paletteHelpers';
import { bitfunSlatePalette } from './slate';

const content = bitfunLightPalette;
const chrome = bitfunSlatePalette;
const paper = STATIC_WHITE;
const ink = chrome.colors.background.primary;

export const bitfunMonochromePalette: AppearancePalette = {
  id: 'bitfun-monochrome',
  name: 'Black & White',
  type: 'light',
  description: 'Black-and-white contrast appearance - Deep black chrome, bright white workspace, soft neutral blocks',
  author: 'BitFun Team',
  version: '1.0.0',

  layout: content.layout,

  colors: {
    background: {
      primary: paper,
      secondary: paper,
      tertiary: content.colors.background.tertiary,
      elevated: paper,
      workbench: paper,
      scene: paper,
    },
    text: content.colors.text,
    accent: createAccentScale({
      base: ink,
      hover: STATIC_BLACK,
      stops: {
        50: content.colors.accent[50],
        100: content.colors.accent[100],
        200: content.colors.accent[200],
        300: content.colors.accent[300],
        400: content.colors.accent[400],
        700: STATIC_BLACK,
      },
    }),
    purple: createSecondaryAccentScale({
      base: ink,
      hover: STATIC_BLACK,
    }),
    semantic: content.colors.semantic,
    border: content.colors.border,
    element: content.colors.element,
    git: content.colors.git,
    scrollbar: {
      thumb: content.colors.border.base,
      thumbHover: content.colors.border.strong,
    },
    chrome: {
      background: {
        primary: chrome.colors.background.primary,
        secondary: chrome.colors.background.secondary,
        tertiary: chrome.colors.background.secondary,
        elevated: chrome.colors.background.elevated,
        workbench: chrome.colors.background.primary,
        scene: chrome.colors.background.primary,
      },
      text: {
        primary: content.colors.element.soft,
        secondary: bitfunDarkPalette.colors.text.secondary,
        muted: content.colors.text.disabled,
        disabled: content.colors.text.muted,
      },
      accent: createAccentScale({
        base: content.colors.element.soft,
        hover: STATIC_WHITE,
        stops: {
          50: bitfunDarkPalette.colors.element.subtle,
          100: bitfunDarkPalette.colors.element.soft,
          200: bitfunDarkPalette.colors.element.base,
          300: bitfunDarkPalette.colors.element.medium,
          400: bitfunDarkPalette.colors.element.strong,
        },
      }),
      border: chrome.colors.border,
      element: chrome.colors.element,
      scrollbar: chrome.colors.scrollbar,
    },
  },

  effects: content.effects,
  motion: content.motion,
  components: {
    button: {
      primary: {
        default: {
          background: ink,
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: STATIC_BLACK,
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: ink,
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
      ghost: {
        default: {
          color: content.colors.text.secondary,
        },
        hover: {
          background: content.colors.element.soft,
          color: ink,
          border: 'transparent',
        },
      },
    },
    configPage: {
      section: {
        background: content.colors.element.soft,
        border: 'transparent',
        borderWidth: '0',
        shadow: 'none',
      },
      divider: content.colors.border.subtle,
      rowHover: content.colors.element.subtle,
    },
  },

  monaco: content.monaco,
};
