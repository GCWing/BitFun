 

import { ThemeConfig } from '../types';

export const bitfunDarkTheme: ThemeConfig = {
  
  id: 'bitfun-dark',
  name: 'Dark',
  type: 'dark',
  description: 'Default dark theme',
  author: 'BitFun Team',
  version: '2.0.0',
  
  
  colors: {
    background: {
      primary: '#121214',
      secondary: '#18181a',
      tertiary: '#121214',
      quaternary: '#202024',
      elevated: '#18181a',
      workbench: '#121214',
      scene: '#16161a',
      tooltip: 'rgba(30, 30, 32, 0.92)',  
    },
    
    text: {
      primary: '#e8e8e8',
      secondary: '#b0b0b0',
      muted: '#858585',
      disabled: '#555555',
    },
    
    accent: {
      50: 'rgba(96, 165, 250, 0.04)',
      100: 'rgba(96, 165, 250, 0.08)',
      200: 'rgba(96, 165, 250, 0.15)',
      300: 'rgba(96, 165, 250, 0.25)',
      400: 'rgba(96, 165, 250, 0.4)',
      500: '#60a5fa',
      600: '#3b82f6',
      700: 'rgba(59, 130, 246, 0.8)',
      800: 'rgba(59, 130, 246, 0.9)',
    },
    
    purple: {
      50: 'rgba(139, 92, 246, 0.04)',
      100: 'rgba(139, 92, 246, 0.08)',
      200: 'rgba(139, 92, 246, 0.15)',
      300: 'rgba(139, 92, 246, 0.25)',
      400: 'rgba(139, 92, 246, 0.4)',
      500: '#8b5cf6',
      600: '#7c3aed',
      700: 'rgba(124, 58, 237, 0.8)',
      800: 'rgba(124, 58, 237, 0.9)',
    },
    
    semantic: {
      success: '#34d399',
      successBg: 'rgba(52, 211, 153, 0.1)',
      successBorder: 'rgba(52, 211, 153, 0.3)',
      
      warning: '#f59e0b',
      warningBg: 'rgba(245, 158, 11, 0.1)',
      warningBorder: 'rgba(245, 158, 11, 0.3)',
      
      error: '#ef4444',
      errorBg: 'rgba(239, 68, 68, 0.1)',
      errorBorder: 'rgba(239, 68, 68, 0.3)',
      
      info: '#E1AB80',
      infoBg: 'rgba(225, 171, 128, 0.1)',
      infoBorder: 'rgba(225, 171, 128, 0.3)',
      
      
      highlight: '#d4a574',
      highlightBg: 'rgba(212, 165, 116, 0.15)',
    },
    
    border: {
      subtle: 'rgba(255, 255, 255, 0.12)',
      base: 'rgba(255, 255, 255, 0.18)',
      medium: 'rgba(255, 255, 255, 0.24)',
      strong: 'rgba(255, 255, 255, 0.32)',
      prominent: 'rgba(225, 171, 128, 0.50)',
    },
    
    element: {
      subtle: 'rgba(255, 255, 255, 0.06)',
      soft: 'rgba(255, 255, 255, 0.10)',
      base: 'rgba(255, 255, 255, 0.13)',
      medium: 'rgba(255, 255, 255, 0.17)',
      strong: 'rgba(255, 255, 255, 0.21)',
      elevated: 'rgba(255, 255, 255, 0.25)',
    },
    
    git: {
      branch: '#E1AB80',
      branchBg: 'rgba(225, 171, 128, 0.08)',
      changes: 'rgb(245, 158, 11)',
      changesBg: 'rgba(245, 158, 11, 0.1)',
      added: 'rgb(34, 197, 94)',
      addedBg: 'rgba(34, 197, 94, 0.1)',
      deleted: 'rgb(239, 68, 68)',
      deletedBg: 'rgba(239, 68, 68, 0.1)',
      staged: 'rgb(34, 197, 94)',
      stagedBg: 'rgba(34, 197, 94, 0.1)',
    },
    
    scrollbar: {
      thumb: 'rgba(255, 255, 255, 0.15)',
      thumbHover: 'rgba(255, 255, 255, 0.28)',
    },
  },
  
  
  effects: {
    shadow: {
      xs: '0 1px 2px rgba(0, 0, 0, 0.9)',
      sm: '0 2px 4px rgba(0, 0, 0, 0.8)',
      base: '0 4px 8px rgba(0, 0, 0, 0.7)',
      lg: '0 8px 16px rgba(0, 0, 0, 0.6)',
      xl: '0 12px 24px rgba(0, 0, 0, 0.5)',
      '2xl': '0 16px 32px rgba(0, 0, 0, 0.4)',
    },
    
    glow: {
      blue: '0 12px 32px rgba(225, 171, 128, 0.25), 0 6px 16px rgba(225, 171, 128, 0.18), 0 3px 8px rgba(0, 0, 0, 0.1)',
      purple: '0 12px 32px rgba(139, 92, 246, 0.25), 0 6px 16px rgba(124, 58, 237, 0.18), 0 3px 8px rgba(0, 0, 0, 0.1)',
      mixed: '0 12px 32px rgba(225, 171, 128, 0.2), 0 6px 16px rgba(139, 92, 246, 0.15), 0 3px 8px rgba(0, 0, 0, 0.1)',
    },
    
    blur: {
      subtle: 'blur(4px) saturate(1.05)',
      base: 'blur(8px) saturate(1.1)',
      medium: 'blur(12px) saturate(1.2)',
      strong: 'blur(16px) saturate(1.3) brightness(1.1)',
      intense: 'blur(20px) saturate(1.4) brightness(1.15)',
    },
    
    radius: {
      sm: '6px',
      base: '8px',
      lg: '12px',
      xl: '16px',
      '2xl': '20px',
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
      disabled: 0.6,
      hover: 0.8,
      focus: 0.9,
      overlay: 0.4,
    },
  },
  
  
  motion: {
    duration: {
      instant: '0.1s',
      fast: '0.15s',
      base: '0.3s',
      slow: '0.6s',
      lazy: '1s',
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
      sans: "'Noto Sans SC', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'SF Pro Display', Roboto, sans-serif",
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
      sm: '14px',
      base: '15px',
      lg: '16px',
      xl: '18px',
      '2xl': '20px',
      '3xl': '24px',
      '4xl': '30px',
      '5xl': '36px',
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
        dot: 'rgba(225, 171, 128, 0.45)',
        dotShadow: '0 0 4px rgba(225, 171, 128, 0.2)',
        hoverBg: 'rgba(225, 171, 128, 0.12)',
        hoverColor: '#E1AB80',
        hoverBorder: 'rgba(225, 171, 128, 0.2)',
        hoverShadow: '0 2px 8px rgba(225, 171, 128, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.1)',
      },
      maximize: {
        dot: 'rgba(225, 171, 128, 0.45)',
        dotShadow: '0 0 4px rgba(225, 171, 128, 0.2)',
        hoverBg: 'rgba(225, 171, 128, 0.12)',
        hoverColor: '#E1AB80',
        hoverBorder: 'rgba(225, 171, 128, 0.2)',
        hoverShadow: '0 2px 8px rgba(225, 171, 128, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.1)',
      },
      close: {
        dot: 'rgba(239, 68, 68, 0.45)',
        dotShadow: '0 0 4px rgba(239, 68, 68, 0.2)',
        hoverBg: 'rgba(239, 68, 68, 0.12)',
        hoverColor: '#ef4444',
        hoverBorder: 'rgba(239, 68, 68, 0.2)',
        hoverShadow: '0 2px 8px rgba(239, 68, 68, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.1)',
      },
      common: {
        defaultColor: 'rgba(232, 232, 232, 0.9)',
        defaultDot: 'rgba(255, 255, 255, 0.2)',
        disabledDot: 'rgba(255, 255, 255, 0.1)',
        flowGradient: 'linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.05), rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.05), transparent)',
      },
    },
    
    button: {
      
      default: {
        background: 'rgba(255, 255, 255, 0.08)',
        color: '#9a9a9a',
        border: 'transparent',
        shadow: 'none',
      },
      hover: {
        background: 'rgba(255, 255, 255, 0.14)',
        color: '#c8c8c8',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      active: {
        background: 'rgba(255, 255, 255, 0.10)',
        color: '#c8c8c8',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      
      
      primary: {
        default: {
          background: '#E1AB80',
          color: '#000000',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: '#F6D0A3',
          color: '#000000',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: '#F6D0A3',
          color: '#000000',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
      
      
      ghost: {
        default: {
          background: 'transparent',
          color: '#9a9a9a',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(255, 255, 255, 0.10)',
          color: '#c8c8c8',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(255, 255, 255, 0.07)',
          color: '#c8c8c8',
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
    rules: [], 
    colors: {
      background: '#121214',
      foreground: '#e8e8e8',
      lineHighlight: '#18181a',
      selection: 'rgba(225, 171, 128, 0.25)',
      cursor: '#E1AB80',
    },
  },
};





