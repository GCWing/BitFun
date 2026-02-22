 

import { ThemeConfig } from '../types';

export const bitfunSlateTheme: ThemeConfig = {
  
  id: 'bitfun-slate',
  name: 'Slate',
  type: 'dark',
  description: 'Slate gray geometric theme - Deep immersion, high contrast grayscale aesthetics',
  author: 'BitFun Team',
  version: '1.2.3',
  
  
  colors: {
    background: {
      primary: '#1a1c1e',       
      secondary: '#1a1c1e',     
      tertiary: '#1a1c1e',      
      quaternary: '#32363a',    
      elevated: '#1a1c1e',      
      workbench: '#1a1c1e',     
      scene: '#1d2023',
      tooltip: 'rgba(42, 45, 48, 0.96)',
    },
    
    text: {
      primary: '#e4e6e8',       
      secondary: '#b8bbc0',     
      muted: '#8a8d92',         
      disabled: '#5a5d62',      
    },
    
    
    accent: {
      50: 'rgba(107, 155, 213, 0.04)',
      100: 'rgba(107, 155, 213, 0.08)',
      200: 'rgba(107, 155, 213, 0.15)',
      300: 'rgba(107, 155, 213, 0.25)',
      400: 'rgba(107, 155, 213, 0.4)',
      500: '#6b9bd5',           
      600: '#5a8bc4',           
      700: 'rgba(90, 139, 196, 0.8)',
      800: 'rgba(90, 139, 196, 0.9)',
    },
    
    
    purple: {
      50: 'rgba(165, 180, 252, 0.04)',
      100: 'rgba(165, 180, 252, 0.08)',
      200: 'rgba(165, 180, 252, 0.15)',
      300: 'rgba(165, 180, 252, 0.25)',
      400: 'rgba(165, 180, 252, 0.4)',
      500: '#a5b4fc',           
      600: '#8b9adb',           
      700: 'rgba(139, 154, 219, 0.8)',
      800: 'rgba(139, 154, 219, 0.9)',
    },
    
    semantic: {
      success: '#7fb899',       
      successBg: 'rgba(127, 184, 153, 0.1)',
      successBorder: 'rgba(127, 184, 153, 0.3)',
      
      warning: '#d4a574',       
      warningBg: 'rgba(212, 165, 116, 0.1)',
      warningBorder: 'rgba(212, 165, 116, 0.3)',
      
      error: '#c9878d',         
      errorBg: 'rgba(201, 135, 141, 0.1)',
      errorBorder: 'rgba(201, 135, 141, 0.3)',
      
      info: '#6b9bd5',          
      infoBg: 'rgba(107, 155, 213, 0.1)',
      infoBorder: 'rgba(107, 155, 213, 0.3)',
      
      
      highlight: '#d4d6d8',
      highlightBg: 'rgba(212, 214, 216, 0.12)',
    },
    
    border: {
      subtle: 'rgba(255, 255, 255, 0.12)',    
      base: 'rgba(255, 255, 255, 0.18)',      
      medium: 'rgba(255, 255, 255, 0.24)',    
      strong: 'rgba(255, 255, 255, 0.32)',    
      prominent: 'rgba(255, 255, 255, 0.45)',  
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
      branch: '#6b9bd5',
      branchBg: 'rgba(107, 155, 213, 0.1)',
      changes: 'rgb(212, 165, 116)',
      changesBg: 'rgba(212, 165, 116, 0.1)',
      added: 'rgb(127, 184, 153)',
      addedBg: 'rgba(127, 184, 153, 0.1)',
      deleted: 'rgb(201, 135, 141)',
      deletedBg: 'rgba(201, 135, 141, 0.1)',
      staged: 'rgb(127, 184, 153)',
      stagedBg: 'rgba(127, 184, 153, 0.1)',
    },
    
    scrollbar: {
      thumb: 'rgba(168, 171, 176, 0.18)',
      thumbHover: 'rgba(168, 171, 176, 0.32)',
    },
  },
  
  
  effects: {
    shadow: {
      xs: '0 1px 2px rgba(0, 0, 0, 0.85)',
      sm: '0 2px 4px rgba(0, 0, 0, 0.8)',
      base: '0 4px 8px rgba(0, 0, 0, 0.75)',
      lg: '0 8px 16px rgba(0, 0, 0, 0.7)',
      xl: '0 12px 24px rgba(0, 0, 0, 0.85)',
      '2xl': '0 16px 32px rgba(0, 0, 0, 0.9)',
    },
    
    glow: {
      blue: '0 12px 32px rgba(107, 155, 213, 0.22), 0 6px 16px rgba(107, 155, 213, 0.15), 0 3px 8px rgba(0, 0, 0, 0.2)',
      purple: '0 12px 32px rgba(165, 180, 252, 0.22), 0 6px 16px rgba(165, 180, 252, 0.15), 0 3px 8px rgba(0, 0, 0, 0.2)',
      mixed: '0 12px 32px rgba(107, 155, 213, 0.18), 0 6px 16px rgba(165, 180, 252, 0.15), 0 3px 8px rgba(0, 0, 0, 0.2)',
    },
    
    blur: {
      subtle: 'blur(4px) saturate(1.05) brightness(0.98)',
      base: 'blur(8px) saturate(1.08) brightness(0.98)',
      medium: 'blur(12px) saturate(1.12) brightness(0.97)',
      strong: 'blur(16px) saturate(1.15) brightness(0.97)',
      intense: 'blur(20px) saturate(1.18) brightness(0.96)',
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
      disabled: 0.5,
      hover: 0.75,
      focus: 0.85,
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
        dot: 'rgba(107, 155, 213, 0.5)',
        dotShadow: '0 0 4px rgba(107, 155, 213, 0.25)',
        hoverBg: 'rgba(107, 155, 213, 0.15)',
        hoverColor: '#6b9bd5',
        hoverBorder: 'rgba(107, 155, 213, 0.25)',
        hoverShadow: '0 2px 8px rgba(107, 155, 213, 0.18), inset 0 1px 0 rgba(255, 255, 255, 0.08)',
      },
      maximize: {
        dot: 'rgba(107, 155, 213, 0.5)',
        dotShadow: '0 0 4px rgba(107, 155, 213, 0.25)',
        hoverBg: 'rgba(107, 155, 213, 0.15)',
        hoverColor: '#6b9bd5',
        hoverBorder: 'rgba(107, 155, 213, 0.25)',
        hoverShadow: '0 2px 8px rgba(107, 155, 213, 0.18), inset 0 1px 0 rgba(255, 255, 255, 0.08)',
      },
      close: {
        dot: 'rgba(201, 135, 141, 0.5)',
        dotShadow: '0 0 4px rgba(201, 135, 141, 0.25)',
        hoverBg: 'rgba(201, 135, 141, 0.15)',
        hoverColor: '#c9878d',
        hoverBorder: 'rgba(201, 135, 141, 0.25)',
        hoverShadow: '0 2px 8px rgba(201, 135, 141, 0.18), inset 0 1px 0 rgba(255, 255, 255, 0.08)',
      },
      common: {
        defaultColor: 'rgba(212, 214, 216, 0.85)',
        defaultDot: 'rgba(168, 171, 176, 0.25)',
        disabledDot: 'rgba(168, 171, 176, 0.12)',
        flowGradient: 'linear-gradient(90deg, transparent, rgba(168, 171, 176, 0.06), rgba(168, 171, 176, 0.10), rgba(168, 171, 176, 0.06), transparent)',
      },
    },
    
    button: {
      
      default: {
        background: 'rgba(168, 171, 176, 0.12)',
        color: '#a8abb0',
        border: 'transparent',
        shadow: 'none',
      },
      hover: {
        background: 'rgba(168, 171, 176, 0.18)',
        color: '#c4c6c9',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      active: {
        background: 'rgba(168, 171, 176, 0.15)',
        color: '#c4c6c9',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      
      
      primary: {
        default: {
          background: 'rgba(107, 155, 213, 0.22)',
          color: '#6b9bd5',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(107, 155, 213, 0.32)',
          color: '#8bb0e0',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(107, 155, 213, 0.26)',
          color: '#8bb0e0',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
      
      
      ghost: {
        default: {
          background: 'transparent',
          color: '#a8abb0',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(168, 171, 176, 0.14)',
          color: '#c4c6c9',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(168, 171, 176, 0.11)',
          color: '#c4c6c9',
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
      { token: 'comment', foreground: '8a8d92', fontStyle: 'italic' },
      { token: 'keyword', foreground: '6b9bd5' },
      { token: 'string', foreground: '8fc8a9' },
      { token: 'number', foreground: 'b5c4fc' },
      { token: 'type', foreground: 'e4b584' },
      { token: 'class', foreground: 'e4b584' },
      { token: 'function', foreground: '8bb0e0' },
      { token: 'variable', foreground: 'b8bbc0' },
      { token: 'constant', foreground: 'b5c4fc' },
      { token: 'operator', foreground: '6b9bd5' },
      { token: 'tag', foreground: 'e4b584' },
      { token: 'attribute.name', foreground: 'b8bbc0' },
      { token: 'attribute.value', foreground: '8fc8a9' },
    ],
    colors: {
      background: '#1a1c1e',
      foreground: '#e4e6e8',
      lineHighlight: '#22252a',
      selection: 'rgba(107, 155, 213, 0.3)',
      cursor: '#6b9bd5',
    },
  },
};
