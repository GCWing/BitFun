

import { ThemeConfig } from '../types';

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
      primary: '#1a1b26',
      secondary: '#16161e',
      tertiary: '#14141b',
      quaternary: '#1e202e',
      elevated: '#20222c',
      workbench: '#16161e',
      scene: '#1a1b26',
      tooltip: 'rgba(22, 22, 30, 0.94)',
    },

    text: {
      primary: '#c0caf5',
      secondary: '#a9b1d6',
      muted: '#787c99',
      disabled: '#545c7e',
    },

    accent: {
      50: 'rgba(122, 162, 247, 0.05)',
      100: 'rgba(122, 162, 247, 0.08)',
      200: 'rgba(122, 162, 247, 0.15)',
      300: 'rgba(122, 162, 247, 0.25)',
      400: 'rgba(122, 162, 247, 0.4)',
      500: '#7aa2f7',
      600: '#6183bb',
      700: 'rgba(97, 131, 187, 0.85)',
      800: 'rgba(97, 131, 187, 0.95)',
    },

    purple: {
      50: 'rgba(187, 154, 247, 0.05)',
      100: 'rgba(187, 154, 247, 0.08)',
      200: 'rgba(187, 154, 247, 0.15)',
      300: 'rgba(187, 154, 247, 0.25)',
      400: 'rgba(187, 154, 247, 0.4)',
      500: '#bb9af7',
      600: '#9d7cd8',
      700: 'rgba(157, 124, 216, 0.85)',
      800: 'rgba(157, 124, 216, 0.95)',
    },

    semantic: {
      success: '#9ece6a',
      successBg: 'rgba(158, 206, 106, 0.12)',
      successBorder: 'rgba(158, 206, 106, 0.35)',

      warning: '#e0af68',
      warningBg: 'rgba(224, 175, 104, 0.12)',
      warningBorder: 'rgba(224, 175, 104, 0.35)',

      error: '#f7768e',
      errorBg: 'rgba(247, 118, 142, 0.12)',
      errorBorder: 'rgba(247, 118, 142, 0.35)',

      info: '#7dcfff',
      infoBg: 'rgba(125, 207, 255, 0.12)',
      infoBorder: 'rgba(125, 207, 255, 0.35)',

      highlight: '#e0af68',
      highlightBg: 'rgba(224, 175, 104, 0.15)',
    },

    border: {
      subtle: 'rgba(54, 59, 84, 0.45)',
      base: 'rgba(54, 59, 84, 0.6)',
      medium: 'rgba(54, 59, 84, 0.72)',
      strong: 'rgba(54, 59, 84, 0.85)',
      prominent: 'rgba(122, 162, 247, 0.45)',
    },

    element: {
      subtle: 'rgba(122, 162, 247, 0.06)',
      soft: 'rgba(122, 162, 247, 0.08)',
      base: 'rgba(122, 162, 247, 0.11)',
      medium: 'rgba(122, 162, 247, 0.14)',
      strong: 'rgba(122, 162, 247, 0.18)',
      elevated: 'rgba(122, 162, 247, 0.22)',
    },

    git: {
      branch: 'rgb(122, 162, 247)',
      branchBg: 'rgba(122, 162, 247, 0.12)',
      changes: 'rgb(224, 175, 104)',
      changesBg: 'rgba(224, 175, 104, 0.12)',
      added: 'rgb(65, 166, 181)',
      addedBg: 'rgba(65, 166, 181, 0.12)',
      deleted: 'rgb(247, 118, 142)',
      deletedBg: 'rgba(247, 118, 142, 0.12)',
      staged: 'rgb(158, 206, 106)',
      stagedBg: 'rgba(158, 206, 106, 0.12)',
    },

    scrollbar: {
      thumb: 'rgba(134, 139, 196, 0.15)',
      thumbHover: 'rgba(134, 139, 196, 0.28)',
    },
  },

  effects: {
    shadow: {
      xs: '0 1px 3px rgba(0, 0, 0, 0.55)',
      sm: '0 2px 6px rgba(0, 0, 0, 0.5)',
      base: '0 4px 12px rgba(0, 0, 0, 0.48)',
      lg: '0 8px 20px rgba(0, 0, 0, 0.45)',
      xl: '0 12px 28px rgba(0, 0, 0, 0.42)',
      '2xl': '0 16px 36px rgba(0, 0, 0, 0.38)',
    },

    glow: {
      blue:
        '0 0 12px rgba(122, 162, 247, 0.35), 0 0 24px rgba(122, 162, 247, 0.2), 0 4px 16px rgba(0, 0, 0, 0.35)',
      purple:
        '0 0 12px rgba(187, 154, 247, 0.32), 0 0 24px rgba(187, 154, 247, 0.18), 0 4px 16px rgba(0, 0, 0, 0.35)',
      mixed:
        '0 0 16px rgba(122, 162, 247, 0.3), 0 0 28px rgba(187, 154, 247, 0.2), 0 4px 20px rgba(0, 0, 0, 0.35)',
    },

    blur: {
      subtle: 'blur(4px) saturate(1.15)',
      base: 'blur(8px) saturate(1.2)',
      medium: 'blur(12px) saturate(1.25)',
      strong: 'blur(16px) saturate(1.3) brightness(1.08)',
      intense: 'blur(20px) saturate(1.35) brightness(1.1)',
    },

    radius: {
      sm: '4px',
      base: '6px',
      lg: '10px',
      xl: '14px',
      '2xl': '18px',
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
      disabled: 0.5,
      hover: 0.88,
      focus: 0.96,
      overlay: 0.52,
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

    easing: {
      standard: 'cubic-bezier(0.4, 0, 0.2, 1)',
      decelerate: 'cubic-bezier(0, 0, 0.2, 1)',
      accelerate: 'cubic-bezier(0.4, 0, 1, 1)',
      bounce: 'cubic-bezier(0.68, -0.55, 0.265, 1.55)',
      smooth: 'cubic-bezier(0.25, 0.46, 0.45, 0.94)',
    },
  },

  typography: {
    font: {
      sans:
        "'Noto Sans SC', -apple-system, BlinkMacSystemFont, 'PingFang SC', 'Hiragino Sans GB', 'Segoe UI', 'Microsoft YaHei UI', 'Microsoft YaHei', 'Helvetica Neue', Helvetica, Arial, sans-serif",
      mono:
        "'JetBrains Mono', 'FiraCode', ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Monaco, 'Cascadia Mono', 'Cascadia Code', Consolas, 'Liberation Mono', 'Courier New', monospace",
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
      tight: 1.3,
      base: 1.5,
      relaxed: 1.65,
    },
  },

  components: {
    windowControls: {
      minimize: {
        dot: 'rgba(122, 162, 247, 0.55)',
        dotShadow: '0 0 6px rgba(122, 162, 247, 0.35)',
        hoverBg: 'rgba(122, 162, 247, 0.14)',
        hoverColor: '#7aa2f7',
        hoverBorder: 'rgba(122, 162, 247, 0.35)',
        hoverShadow:
          '0 0 12px rgba(122, 162, 247, 0.28), 0 2px 8px rgba(122, 162, 247, 0.15), inset 0 1px 0 rgba(122, 162, 247, 0.18)',
      },
      maximize: {
        dot: 'rgba(122, 162, 247, 0.55)',
        dotShadow: '0 0 6px rgba(122, 162, 247, 0.35)',
        hoverBg: 'rgba(122, 162, 247, 0.14)',
        hoverColor: '#7aa2f7',
        hoverBorder: 'rgba(122, 162, 247, 0.35)',
        hoverShadow:
          '0 0 12px rgba(122, 162, 247, 0.28), 0 2px 8px rgba(122, 162, 247, 0.15), inset 0 1px 0 rgba(122, 162, 247, 0.18)',
      },
      close: {
        dot: 'rgba(247, 118, 142, 0.55)',
        dotShadow: '0 0 6px rgba(247, 118, 142, 0.35)',
        hoverBg: 'rgba(247, 118, 142, 0.14)',
        hoverColor: '#f7768e',
        hoverBorder: 'rgba(247, 118, 142, 0.35)',
        hoverShadow:
          '0 0 12px rgba(247, 118, 142, 0.28), 0 2px 8px rgba(247, 118, 142, 0.15), inset 0 1px 0 rgba(247, 118, 142, 0.18)',
      },
      common: {
        defaultColor: 'rgba(192, 202, 245, 0.92)',
        defaultDot: 'rgba(122, 162, 247, 0.22)',
        disabledDot: 'rgba(122, 162, 247, 0.12)',
        flowGradient:
          'linear-gradient(90deg, transparent, rgba(122, 162, 247, 0.08), rgba(187, 154, 247, 0.1), rgba(122, 162, 247, 0.08), transparent)',
      },
    },

    button: {
      default: {
        background: 'rgba(122, 162, 247, 0.1)',
        color: '#a9b1d6',
        border: 'rgba(122, 162, 247, 0.22)',
        shadow: '0 0 8px rgba(122, 162, 247, 0.12)',
      },
      hover: {
        background: 'rgba(122, 162, 247, 0.16)',
        color: '#c0caf5',
        border: 'rgba(122, 162, 247, 0.38)',
        shadow: '0 0 16px rgba(122, 162, 247, 0.22), 0 2px 8px rgba(0, 0, 0, 0.35)',
        transform: 'translateY(-1px)',
      },
      active: {
        background: 'rgba(122, 162, 247, 0.13)',
        color: '#c0caf5',
        border: 'rgba(122, 162, 247, 0.42)',
        shadow: '0 0 12px rgba(122, 162, 247, 0.18)',
        transform: 'translateY(0)',
      },

      primary: {
        default: {
          background: 'rgba(61, 89, 161, 0.55)',
          color: '#c0caf5',
          border: 'rgba(122, 162, 247, 0.45)',
          shadow: '0 0 14px rgba(61, 89, 161, 0.35)',
        },
        hover: {
          background: 'rgba(61, 89, 161, 0.72)',
          color: '#ffffff',
          border: 'rgba(122, 162, 247, 0.55)',
          shadow:
            '0 0 22px rgba(122, 162, 247, 0.35), 0 4px 12px rgba(0, 0, 0, 0.35)',
          transform: 'translateY(-2px)',
        },
        active: {
          background: 'rgba(61, 89, 161, 0.62)',
          color: '#ffffff',
          border: 'rgba(122, 162, 247, 0.48)',
          shadow: '0 0 18px rgba(122, 162, 247, 0.28)',
          transform: 'translateY(-1px)',
        },
      },

      ghost: {
        default: {
          background: 'transparent',
          color: '#787c99',
          border: 'rgba(54, 59, 84, 0.55)',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(122, 162, 247, 0.1)',
          color: '#c0caf5',
          border: 'rgba(122, 162, 247, 0.35)',
          shadow: '0 0 12px rgba(122, 162, 247, 0.15)',
          transform: 'translateY(-1px)',
        },
        active: {
          background: 'rgba(122, 162, 247, 0.08)',
          color: '#a9b1d6',
          border: 'rgba(122, 162, 247, 0.3)',
          shadow: '0 0 8px rgba(122, 162, 247, 0.12)',
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
      background: '#1a1b26',
      foreground: '#a9b1d6',
      lineHighlight: '#1e202e',
      selection: 'rgba(81, 92, 126, 0.35)',
      cursor: '#c0caf5',
    },
  },
};
