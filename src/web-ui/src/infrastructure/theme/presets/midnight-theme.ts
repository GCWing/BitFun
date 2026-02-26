 

import { ThemeConfig } from '../types';

export const bitfunMidnightTheme: ThemeConfig = {
  
  id: 'bitfun-midnight',
  name: 'Midnight',
  type: 'dark',
  description: 'Midnight gray dark theme - Professional and elegant, inspired by JetBrains IDE',
  author: 'BitFun Team',
  version: '1.0.0',
  
  
  colors: {
    background: {
      primary: '#2b2d30',      
      secondary: '#1e1f22',    
      tertiary: '#313335',     
      quaternary: '#3c3f41',   
      elevated: '#2b2d30',     
      workbench: '#212121',    
      scene: '#27292c',
      tooltip: 'rgba(43, 45, 48, 0.94)',
    },
    
    text: {
      primary: '#bcbec4',      
      secondary: '#9da0a8',    
      muted: '#6f737a',        
      disabled: '#4e5157',     
    },
    
    accent: {
      50: 'rgba(88, 166, 255, 0.04)',
      100: 'rgba(88, 166, 255, 0.08)',
      200: 'rgba(88, 166, 255, 0.15)',
      300: 'rgba(88, 166, 255, 0.25)',
      400: 'rgba(88, 166, 255, 0.4)',
      500: '#58a6ff',          
      600: '#3b82f6',          
      700: 'rgba(59, 130, 246, 0.8)',
      800: 'rgba(59, 130, 246, 0.9)',
    },
    
    purple: {
      50: 'rgba(156, 120, 255, 0.04)',
      100: 'rgba(156, 120, 255, 0.08)',
      200: 'rgba(156, 120, 255, 0.15)',
      300: 'rgba(156, 120, 255, 0.25)',
      400: 'rgba(156, 120, 255, 0.4)',
      500: '#9c78ff',          
      600: '#8b5cf6',          
      700: 'rgba(139, 92, 246, 0.8)',
      800: 'rgba(139, 92, 246, 0.9)',
    },
    
    semantic: {
      success: '#6aab73',      
      successBg: 'rgba(106, 171, 115, 0.1)',
      successBorder: 'rgba(106, 171, 115, 0.3)',
      
      warning: '#e0a055',      
      warningBg: 'rgba(224, 160, 85, 0.1)',
      warningBorder: 'rgba(224, 160, 85, 0.3)',
      
      error: '#cc7f7a',        
      errorBg: 'rgba(204, 127, 122, 0.1)',
      errorBorder: 'rgba(204, 127, 122, 0.3)',
      
      info: '#58a6ff',         
      infoBg: 'rgba(88, 166, 255, 0.1)',
      infoBorder: 'rgba(88, 166, 255, 0.3)',
      
      
      highlight: '#d4a574',
      highlightBg: 'rgba(212, 165, 116, 0.15)',
    },
    
    border: {
      subtle: 'rgba(255, 255, 255, 0.08)',
      base: 'rgba(255, 255, 255, 0.14)',
      medium: 'rgba(255, 255, 255, 0.20)',
      strong: 'rgba(255, 255, 255, 0.26)',
      prominent: 'rgba(255, 255, 255, 0.35)',
    },
    
    element: {
      subtle: 'rgba(255, 255, 255, 0.04)',
      soft: 'rgba(255, 255, 255, 0.06)',
      base: 'rgba(255, 255, 255, 0.09)',
      medium: 'rgba(255, 255, 255, 0.12)',
      strong: 'rgba(255, 255, 255, 0.15)',
      elevated: 'rgba(255, 255, 255, 0.18)',
    },
    
    git: {
      branch: 'rgb(88, 166, 255)',
      branchBg: 'rgba(88, 166, 255, 0.1)',
      changes: 'rgb(224, 160, 85)',
      changesBg: 'rgba(224, 160, 85, 0.1)',
      added: 'rgb(106, 171, 115)',
      addedBg: 'rgba(106, 171, 115, 0.1)',
      deleted: 'rgb(204, 127, 122)',
      deletedBg: 'rgba(204, 127, 122, 0.1)',
      staged: 'rgb(106, 171, 115)',
      stagedBg: 'rgba(106, 171, 115, 0.1)',
    },
  },
  
  
  effects: {
    shadow: {
      xs: '0 1px 2px rgba(0, 0, 0, 0.8)',
      sm: '0 2px 4px rgba(0, 0, 0, 0.75)',
      base: '0 4px 8px rgba(0, 0, 0, 0.7)',
      lg: '0 8px 16px rgba(0, 0, 0, 0.65)',
      xl: '0 12px 24px rgba(0, 0, 0, 0.8)',
      '2xl': '0 16px 32px rgba(0, 0, 0, 0.85)',
    },
    
    glow: {
      blue: '0 12px 32px rgba(88, 166, 255, 0.25), 0 6px 16px rgba(88, 166, 255, 0.18), 0 3px 8px rgba(0, 0, 0, 0.15)',
      purple: '0 12px 32px rgba(156, 120, 255, 0.25), 0 6px 16px rgba(156, 120, 255, 0.18), 0 3px 8px rgba(0, 0, 0, 0.15)',
      mixed: '0 12px 32px rgba(88, 166, 255, 0.2), 0 6px 16px rgba(156, 120, 255, 0.18), 0 3px 8px rgba(0, 0, 0, 0.15)',
    },
    
    blur: {
      subtle: 'blur(4px) saturate(1.1)',
      base: 'blur(8px) saturate(1.2)',
      medium: 'blur(12px) saturate(1.25)',
      strong: 'blur(16px) saturate(1.3) brightness(1.05)',
      intense: 'blur(20px) saturate(1.4) brightness(1.1)',
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
      disabled: 0.5,
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
        dot: 'rgba(88, 166, 255, 0.45)',
        dotShadow: '0 0 4px rgba(88, 166, 255, 0.2)',
        hoverBg: 'rgba(88, 166, 255, 0.12)',
        hoverColor: '#58a6ff',
        hoverBorder: 'rgba(88, 166, 255, 0.2)',
        hoverShadow: '0 2px 8px rgba(88, 166, 255, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.1)',
      },
      maximize: {
        dot: 'rgba(88, 166, 255, 0.45)',
        dotShadow: '0 0 4px rgba(88, 166, 255, 0.2)',
        hoverBg: 'rgba(88, 166, 255, 0.12)',
        hoverColor: '#58a6ff',
        hoverBorder: 'rgba(88, 166, 255, 0.2)',
        hoverShadow: '0 2px 8px rgba(88, 166, 255, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.1)',
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
        defaultColor: 'rgba(188, 190, 196, 0.9)',
        defaultDot: 'rgba(188, 190, 196, 0.2)',
        disabledDot: 'rgba(188, 190, 196, 0.1)',
        flowGradient: 'linear-gradient(90deg, transparent, rgba(188, 190, 196, 0.05), rgba(188, 190, 196, 0.08), rgba(188, 190, 196, 0.05), transparent)',
      },
    },
    
    button: {
      
      default: {
        background: 'rgba(188, 190, 196, 0.11)',
        color: '#949699',
        border: 'transparent',
        shadow: 'none',
      },
      hover: {
        background: 'rgba(188, 190, 196, 0.17)',
        color: '#afb1b5',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      active: {
        background: 'rgba(188, 190, 196, 0.14)',
        color: '#afb1b5',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      
      
      primary: {
        default: {
          background: 'rgba(88, 166, 255, 0.20)',
          color: '#6aa8e8',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(88, 166, 255, 0.30)',
          color: '#8fc0f0',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(88, 166, 255, 0.24)',
          color: '#8fc0f0',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
      
      
      ghost: {
        default: {
          background: 'transparent',
          color: '#949699',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(188, 190, 196, 0.13)',
          color: '#afb1b5',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(188, 190, 196, 0.11)',
          color: '#afb1b5',
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
      { token: 'comment', foreground: '6f737a', fontStyle: 'italic' },
      { token: 'keyword', foreground: 'cc7832' },
      { token: 'string', foreground: '6aab73' },
      { token: 'number', foreground: '6897bb' },
      { token: 'type', foreground: 'e0a055' },
      { token: 'class', foreground: 'e0a055' },
      { token: 'function', foreground: 'ffc66d' },
      { token: 'variable', foreground: 'bcbec4' },
      { token: 'constant', foreground: '9876aa' },
      { token: 'operator', foreground: 'cc7832' },
      { token: 'tag', foreground: 'e8bf6a' },
      { token: 'attribute.name', foreground: 'bababa' },
      { token: 'attribute.value', foreground: 'a5c261' },
    ],
    colors: {
      background: '#2b2d30',
      foreground: '#bcbec4',
      lineHighlight: '#313335',
      selection: '#3d4752',
      cursor: '#58a6ff',
    },
  },
};

