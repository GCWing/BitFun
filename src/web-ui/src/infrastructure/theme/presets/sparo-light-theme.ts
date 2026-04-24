import { ThemeConfig } from '../types';

export const sparoLightTheme: ThemeConfig = {
  id: 'sparo-light',
  name: 'Sparo Light',
  type: 'light',
  description: 'Sparo brand light theme - Calm cloud surfaces with ink text and print-red signals',
  author: 'BitFun Team',
  version: '1.0.0',

  layout: {
    sceneViewportBorder: false,
  },

  colors: {
    background: {
      primary: '#F8FAFC',
      secondary: '#FFFFFF',
      tertiary: '#F1F5F9',
      quaternary: '#E7ECF3',
      elevated: '#FFFFFF',
      workbench: '#F3F6FA',
      scene: '#FFFFFF',
      tooltip: 'rgba(255, 255, 255, 0.98)',
    },

    text: {
      primary: '#0F172A',
      secondary: '#253044',
      muted: '#5B6B8C',
      disabled: 'rgba(91, 107, 140, 0.56)',
    },

    accent: {
      50: 'rgba(183, 55, 47, 0.045)',
      100: 'rgba(183, 55, 47, 0.08)',
      200: 'rgba(183, 55, 47, 0.13)',
      300: 'rgba(183, 55, 47, 0.22)',
      400: 'rgba(183, 55, 47, 0.42)',
      500: '#B7372F',
      600: '#9E2F29',
      700: 'rgba(158, 47, 41, 0.88)',
      800: 'rgba(128, 36, 32, 0.94)',
    },

    purple: {
      50: 'rgba(91, 107, 140, 0.045)',
      100: 'rgba(91, 107, 140, 0.08)',
      200: 'rgba(91, 107, 140, 0.13)',
      300: 'rgba(91, 107, 140, 0.2)',
      400: 'rgba(91, 107, 140, 0.34)',
      500: '#5B6B8C',
      600: '#253044',
      700: 'rgba(37, 48, 68, 0.82)',
      800: 'rgba(15, 23, 42, 0.9)',
    },

    semantic: {
      success: '#4F8F67',
      successBg: 'rgba(79, 143, 103, 0.09)',
      successBorder: 'rgba(79, 143, 103, 0.25)',

      warning: '#B8842B',
      warningBg: 'rgba(184, 132, 43, 0.1)',
      warningBorder: 'rgba(184, 132, 43, 0.28)',

      error: '#B7372F',
      errorBg: 'rgba(183, 55, 47, 0.09)',
      errorBorder: 'rgba(183, 55, 47, 0.28)',

      info: '#5B6B8C',
      infoBg: 'rgba(91, 107, 140, 0.1)',
      infoBorder: 'rgba(91, 107, 140, 0.25)',

      highlight: '#B7372F',
      highlightBg: 'rgba(183, 55, 47, 0.11)',
    },

    border: {
      subtle: 'rgba(15, 23, 42, 0.08)',
      base: 'rgba(15, 23, 42, 0.12)',
      medium: 'rgba(15, 23, 42, 0.18)',
      strong: 'rgba(15, 23, 42, 0.28)',
      prominent: 'rgba(15, 23, 42, 0.38)',
    },

    element: {
      subtle: 'rgba(15, 23, 42, 0.035)',
      soft: 'rgba(15, 23, 42, 0.055)',
      base: 'rgba(15, 23, 42, 0.075)',
      medium: 'rgba(15, 23, 42, 0.105)',
      strong: 'rgba(15, 23, 42, 0.145)',
      elevated: 'rgba(255, 255, 255, 0.94)',
    },

    git: {
      branch: '#5B6B8C',
      branchBg: 'rgba(91, 107, 140, 0.1)',
      changes: '#B8842B',
      changesBg: 'rgba(184, 132, 43, 0.09)',
      added: '#4F8F67',
      addedBg: 'rgba(79, 143, 103, 0.09)',
      deleted: '#B7372F',
      deletedBg: 'rgba(183, 55, 47, 0.09)',
      staged: '#4F8F67',
      stagedBg: 'rgba(79, 143, 103, 0.09)',
    },

    scrollbar: {
      thumb: 'rgba(15, 23, 42, 0.14)',
      thumbHover: 'rgba(15, 23, 42, 0.28)',
    },
  },

  effects: {
    shadow: {
      xs: '0 1px 2px rgba(15, 23, 42, 0.04)',
      sm: '0 2px 5px rgba(15, 23, 42, 0.06)',
      base: '0 5px 12px rgba(15, 23, 42, 0.08)',
      lg: '0 10px 22px rgba(15, 23, 42, 0.1)',
      xl: '0 16px 34px rgba(15, 23, 42, 0.12)',
      '2xl': '0 22px 48px rgba(15, 23, 42, 0.14)',
    },

    glow: {
      blue: '0 10px 28px rgba(15, 23, 42, 0.06), 0 4px 14px rgba(91, 107, 140, 0.06)',
      purple: '0 10px 28px rgba(91, 107, 140, 0.08), 0 4px 14px rgba(15, 23, 42, 0.05)',
      mixed: '0 10px 28px rgba(183, 55, 47, 0.08), 0 4px 14px rgba(15, 23, 42, 0.05)',
    },

    blur: {
      subtle: 'blur(4px) saturate(1.02)',
      base: 'blur(8px) saturate(1.04)',
      medium: 'blur(12px) saturate(1.06)',
      strong: 'blur(16px) saturate(1.08) brightness(1.02)',
      intense: 'blur(20px) saturate(1.1) brightness(1.03)',
    },

    radius: {
      sm: '4px',
      base: '6px',
      lg: '8px',
      xl: '12px',
      '2xl': '16px',
      full: '9999px',
    },

    spacing: {
      1: '4px',
      2: '8px',
      3: '12px',
      4: '16px',
      5: '20px',
      6: '24px',
      8: '32px',
      10: '40px',
      12: '48px',
      16: '64px',
    },

    opacity: {
      disabled: 0.55,
      hover: 0.75,
      focus: 0.9,
      overlay: 0.32,
    },
  },

  motion: {
    duration: {
      instant: '0.1s',
      fast: '0.15s',
      base: '0.28s',
      slow: '0.55s',
      lazy: '0.9s',
    },

    easing: {
      standard: 'cubic-bezier(0.4, 0, 0.2, 1)',
      decelerate: 'cubic-bezier(0, 0, 0.2, 1)',
      accelerate: 'cubic-bezier(0.4, 0, 1, 1)',
      bounce: 'cubic-bezier(0.68, -0.55, 0.265, 1.55)',
      smooth: 'cubic-bezier(0.4, 0, 0.2, 1)',
    },
  },

  typography: {
    font: {
      sans: "'Inter', 'Geist', 'Noto Sans SC', 'HarmonyOS Sans SC', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
      mono: "'FiraCode', 'JetBrains Mono', 'SF Mono', 'Consolas', 'Liberation Mono', monospace",
    },

    weight: {
      normal: 400,
      medium: 500,
      semibold: 600,
      bold: 700,
    },

    size: {
      xs: '12px',
      sm: '13px',
      base: '14px',
      lg: '15px',
      xl: '16px',
      '2xl': '18px',
      '3xl': '22px',
      '4xl': '26px',
      '5xl': '32px',
    },

    lineHeight: {
      tight: 1.2,
      base: 1.5,
      relaxed: 1.6,
    },
  },

  components: {
    windowControls: {
      minimize: {
        dot: 'rgba(91, 107, 140, 0.5)',
        dotShadow: '0 0 4px rgba(15, 23, 42, 0.1)',
        hoverBg: 'rgba(15, 23, 42, 0.07)',
        hoverColor: '#253044',
        hoverBorder: 'rgba(15, 23, 42, 0.14)',
        hoverShadow: '0 2px 8px rgba(15, 23, 42, 0.07), inset 0 1px 0 rgba(255, 255, 255, 0.9)',
      },
      maximize: {
        dot: 'rgba(91, 107, 140, 0.5)',
        dotShadow: '0 0 4px rgba(15, 23, 42, 0.1)',
        hoverBg: 'rgba(15, 23, 42, 0.07)',
        hoverColor: '#253044',
        hoverBorder: 'rgba(15, 23, 42, 0.14)',
        hoverShadow: '0 2px 8px rgba(15, 23, 42, 0.07), inset 0 1px 0 rgba(255, 255, 255, 0.9)',
      },
      close: {
        dot: 'rgba(183, 55, 47, 0.55)',
        dotShadow: '0 0 4px rgba(183, 55, 47, 0.18)',
        hoverBg: 'rgba(183, 55, 47, 0.12)',
        hoverColor: '#B7372F',
        hoverBorder: 'rgba(183, 55, 47, 0.24)',
        hoverShadow: '0 2px 8px rgba(183, 55, 47, 0.14), inset 0 1px 0 rgba(255, 255, 255, 0.85)',
      },
      common: {
        defaultColor: 'rgba(15, 23, 42, 0.92)',
        defaultDot: 'rgba(91, 107, 140, 0.28)',
        disabledDot: 'rgba(91, 107, 140, 0.16)',
        flowGradient: 'linear-gradient(90deg, transparent, rgba(91, 107, 140, 0.05), rgba(183, 55, 47, 0.08), rgba(91, 107, 140, 0.05), transparent)',
      },
    },

    button: {
      default: {
        background: 'rgba(15, 23, 42, 0.06)',
        color: '#253044',
        border: 'transparent',
        shadow: 'none',
      },
      hover: {
        background: 'rgba(15, 23, 42, 0.1)',
        color: '#0F172A',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      active: {
        background: 'rgba(15, 23, 42, 0.08)',
        color: '#0F172A',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },

      primary: {
        default: {
          background: '#B7372F',
          color: '#FFFFFF',
          border: 'transparent',
          shadow: '0 2px 8px rgba(183, 55, 47, 0.18)',
        },
        hover: {
          background: '#9E2F29',
          color: '#FFFFFF',
          border: 'transparent',
          shadow: '0 4px 14px rgba(183, 55, 47, 0.22)',
          transform: 'none',
        },
        active: {
          background: '#802420',
          color: '#FFFFFF',
          border: 'transparent',
          shadow: '0 2px 8px rgba(183, 55, 47, 0.18)',
          transform: 'none',
        },
      },

      ghost: {
        default: {
          background: 'transparent',
          color: '#253044',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(15, 23, 42, 0.07)',
          color: '#0F172A',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(15, 23, 42, 0.05)',
          color: '#0F172A',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
    },
  },

  monaco: {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '6B7890', fontStyle: 'italic' },
      { token: 'keyword', foreground: '9E2F29' },
      { token: 'string', foreground: '4F8F67' },
      { token: 'number', foreground: 'B8842B' },
      { token: 'type', foreground: '253044' },
      { token: 'class', foreground: '253044' },
      { token: 'function', foreground: 'B7372F' },
      { token: 'variable', foreground: '253044' },
      { token: 'constant', foreground: 'B8842B' },
      { token: 'operator', foreground: '5B6B8C' },
      { token: 'tag', foreground: '9E2F29' },
      { token: 'attribute.name', foreground: '5B6B8C' },
      { token: 'attribute.value', foreground: '4F8F67' },
    ],
    colors: {
      background: '#F8FAFC',
      foreground: '#0F172A',
      lineHighlight: '#F1F5F9',
      selection: 'rgba(183, 55, 47, 0.16)',
      cursor: '#B7372F',

      'editor.selectionBackground': 'rgba(183, 55, 47, 0.16)',
      'editor.selectionForeground': '#0F172A',
      'editor.inactiveSelectionBackground': 'rgba(15, 23, 42, 0.08)',
      'editor.selectionHighlightBackground': 'rgba(183, 55, 47, 0.1)',
      'editor.selectionHighlightBorder': 'rgba(183, 55, 47, 0.22)',
      'editorCursor.foreground': '#B7372F',

      'editor.wordHighlightBackground': 'rgba(15, 23, 42, 0.06)',
      'editor.wordHighlightStrongBackground': 'rgba(183, 55, 47, 0.1)',
    },
  },
};
