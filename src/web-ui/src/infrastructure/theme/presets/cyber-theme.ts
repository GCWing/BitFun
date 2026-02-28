 

import { ThemeConfig } from '../types';

export const bitfunCyberTheme: ThemeConfig = {
  
  id: 'bitfun-cyber',
  name: 'Cyber',
  type: 'dark',
  description: 'Tech-style theme - Deep black hole, neon future, ultimate tech aesthetics',
  author: 'BitFun Team',
  version: '1.0.0',
  
  
  colors: {
    background: {
      primary: '#101010',        
      secondary: '#151515',      
      tertiary: '#1a1a1a',       
      quaternary: '#1f1f1f',     
      elevated: '#0d0d0d',       
      workbench: '#101010',      
      scene: '#141414',
      tooltip: 'rgba(16, 16, 16, 0.95)',
    },
    
    text: {
      primary: '#e0f2ff',        
      secondary: '#c7e7ff',      
      muted: '#7fadcc',          
      disabled: '#4a5a66',       
    },
    
    accent: {
      50: 'rgba(0, 230, 255, 0.05)',
      100: 'rgba(0, 230, 255, 0.1)',
      200: 'rgba(0, 230, 255, 0.18)',
      300: 'rgba(0, 230, 255, 0.3)',
      400: 'rgba(0, 230, 255, 0.45)',
      500: '#00e6ff',            
      600: '#00ccff',            
      700: 'rgba(0, 204, 255, 0.85)',
      800: 'rgba(0, 204, 255, 0.95)',
    },
    
    purple: {
      50: 'rgba(138, 43, 226, 0.05)',
      100: 'rgba(138, 43, 226, 0.1)',
      200: 'rgba(138, 43, 226, 0.18)',
      300: 'rgba(138, 43, 226, 0.3)',
      400: 'rgba(138, 43, 226, 0.45)',
      500: '#8a2be2',            
      600: '#7928ca',            
      700: 'rgba(121, 40, 202, 0.85)',
      800: 'rgba(121, 40, 202, 0.95)',
    },
    
    semantic: {
      success: '#00ff9f',        
      successBg: 'rgba(0, 255, 159, 0.12)',
      successBorder: 'rgba(0, 255, 159, 0.35)',
      
      warning: '#ffcc00',        
      warningBg: 'rgba(255, 204, 0, 0.12)',
      warningBorder: 'rgba(255, 204, 0, 0.35)',
      
      error: '#ff0055',          
      errorBg: 'rgba(255, 0, 85, 0.12)',
      errorBorder: 'rgba(255, 0, 85, 0.35)',
      
      info: '#00e6ff',           
      infoBg: 'rgba(0, 230, 255, 0.12)',
      infoBorder: 'rgba(0, 230, 255, 0.35)',
      
      
      highlight: '#ffdd44',
      highlightBg: 'rgba(255, 221, 68, 0.15)',
    },
    
    border: {
      subtle: 'rgba(0, 230, 255, 0.14)',
      base: 'rgba(0, 230, 255, 0.20)',
      medium: 'rgba(0, 230, 255, 0.28)',
      strong: 'rgba(0, 230, 255, 0.36)',
      prominent: 'rgba(0, 230, 255, 0.50)',
    },
    
    element: {
      subtle: 'rgba(0, 230, 255, 0.06)',
      soft: 'rgba(0, 230, 255, 0.09)',
      base: 'rgba(0, 230, 255, 0.13)',
      medium: 'rgba(0, 230, 255, 0.17)',
      strong: 'rgba(0, 230, 255, 0.22)',
      elevated: 'rgba(0, 230, 255, 0.27)',
    },
    
    git: {
      branch: 'rgb(0, 230, 255)',
      branchBg: 'rgba(0, 230, 255, 0.12)',
      changes: 'rgb(255, 204, 0)',
      changesBg: 'rgba(255, 204, 0, 0.12)',
      added: 'rgb(0, 255, 159)',
      addedBg: 'rgba(0, 255, 159, 0.12)',
      deleted: 'rgb(255, 0, 85)',
      deletedBg: 'rgba(255, 0, 85, 0.12)',
      staged: 'rgb(0, 255, 159)',
      stagedBg: 'rgba(0, 255, 159, 0.12)',
    },
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
      
      blue: '0 0 12px rgba(0, 230, 255, 0.4), 0 0 24px rgba(0, 230, 255, 0.25), 0 0 36px rgba(0, 230, 255, 0.15), 0 4px 16px rgba(0, 0, 0, 0.3)',
      
      purple: '0 0 12px rgba(138, 43, 226, 0.4), 0 0 24px rgba(138, 43, 226, 0.25), 0 0 36px rgba(138, 43, 226, 0.15), 0 4px 16px rgba(0, 0, 0, 0.3)',
      
      mixed: '0 0 16px rgba(0, 230, 255, 0.35), 0 0 28px rgba(138, 43, 226, 0.25), 0 0 40px rgba(0, 230, 255, 0.12), 0 4px 20px rgba(0, 0, 0, 0.35)',
    },
    
    blur: {
      subtle: 'blur(4px) saturate(1.2)',
      base: 'blur(8px) saturate(1.3)',
      medium: 'blur(12px) saturate(1.4)',
      strong: 'blur(16px) saturate(1.5) brightness(1.15)',
      intense: 'blur(20px) saturate(1.6) brightness(1.2)',
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
      sans: "'Noto Sans SC', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'SF Pro Display', 'Roboto', sans-serif",
      mono: "'FiraCode', 'JetBrains Mono', 'SF Mono', 'Cascadia Code', 'Consolas', monospace",
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
      lg: '16px',
      xl: '18px',
      '2xl': '20px',
      '3xl': '24px',
      '4xl': '30px',
      '5xl': '36px',
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
        dot: 'rgba(0, 230, 255, 0.5)',
        dotShadow: '0 0 6px rgba(0, 230, 255, 0.35)',
        hoverBg: 'rgba(0, 230, 255, 0.15)',
        hoverColor: '#00e6ff',
        hoverBorder: 'rgba(0, 230, 255, 0.3)',
        hoverShadow: '0 0 12px rgba(0, 230, 255, 0.3), 0 2px 8px rgba(0, 230, 255, 0.2), inset 0 1px 0 rgba(0, 230, 255, 0.2)',
      },
      maximize: {
        dot: 'rgba(0, 230, 255, 0.5)',
        dotShadow: '0 0 6px rgba(0, 230, 255, 0.35)',
        hoverBg: 'rgba(0, 230, 255, 0.15)',
        hoverColor: '#00e6ff',
        hoverBorder: 'rgba(0, 230, 255, 0.3)',
        hoverShadow: '0 0 12px rgba(0, 230, 255, 0.3), 0 2px 8px rgba(0, 230, 255, 0.2), inset 0 1px 0 rgba(0, 230, 255, 0.2)',
      },
      close: {
        dot: 'rgba(255, 0, 85, 0.5)',
        dotShadow: '0 0 6px rgba(255, 0, 85, 0.35)',
        hoverBg: 'rgba(255, 0, 85, 0.15)',
        hoverColor: '#ff0055',
        hoverBorder: 'rgba(255, 0, 85, 0.3)',
        hoverShadow: '0 0 12px rgba(255, 0, 85, 0.3), 0 2px 8px rgba(255, 0, 85, 0.2), inset 0 1px 0 rgba(255, 0, 85, 0.2)',
      },
      common: {
        defaultColor: 'rgba(224, 242, 255, 0.9)',
        defaultDot: 'rgba(0, 230, 255, 0.2)',
        disabledDot: 'rgba(0, 230, 255, 0.1)',
        flowGradient: 'linear-gradient(90deg, transparent, rgba(0, 230, 255, 0.08), rgba(0, 230, 255, 0.12), rgba(0, 230, 255, 0.08), transparent)',
      },
    },
    
    button: {
      
      default: {
        background: 'rgba(0, 230, 255, 0.08)',
        color: '#7fadcc',
        border: 'rgba(0, 230, 255, 0.15)',
        shadow: '0 0 8px rgba(0, 230, 255, 0.1)',
      },
      hover: {
        background: 'rgba(0, 230, 255, 0.14)',
        color: '#c7e7ff',
        border: 'rgba(0, 230, 255, 0.3)',
        shadow: '0 0 16px rgba(0, 230, 255, 0.2), 0 2px 8px rgba(0, 0, 0, 0.3)',
        transform: 'translateY(-1px)',
      },
      active: {
        background: 'rgba(0, 230, 255, 0.12)',
        color: '#c7e7ff',
        border: 'rgba(0, 230, 255, 0.35)',
        shadow: '0 0 12px rgba(0, 230, 255, 0.15)',
        transform: 'translateY(0)',
      },
      
      
      primary: {
        default: {
          background: 'rgba(0, 230, 255, 0.18)',
          color: '#e0f2ff',
          border: 'rgba(0, 230, 255, 0.4)',
          shadow: '0 0 16px rgba(0, 230, 255, 0.25)',
        },
        hover: {
          background: 'rgba(0, 230, 255, 0.25)',
          color: '#ffffff',
          border: 'rgba(0, 230, 255, 0.6)',
          shadow: '0 0 24px rgba(0, 230, 255, 0.4), 0 0 36px rgba(0, 230, 255, 0.2), 0 4px 12px rgba(0, 0, 0, 0.3)',
          transform: 'translateY(-2px)',
        },
        active: {
          background: 'rgba(0, 230, 255, 0.22)',
          color: '#ffffff',
          border: 'rgba(0, 230, 255, 0.5)',
          shadow: '0 0 20px rgba(0, 230, 255, 0.3)',
          transform: 'translateY(-1px)',
        },
      },
      
      
      ghost: {
        default: {
          background: 'transparent',
          color: '#7fadcc',
          border: 'rgba(0, 230, 255, 0.2)',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(0, 230, 255, 0.1)',
          color: '#c7e7ff',
          border: 'rgba(0, 230, 255, 0.35)',
          shadow: '0 0 12px rgba(0, 230, 255, 0.15)',
          transform: 'translateY(-1px)',
        },
        active: {
          background: 'rgba(0, 230, 255, 0.08)',
          color: '#c7e7ff',
          border: 'rgba(0, 230, 255, 0.3)',
          shadow: '0 0 8px rgba(0, 230, 255, 0.1)',
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
      background: '#101010',
      foreground: '#c7e7ff',
      lineHighlight: '#151515',
      selection: '#1a4d66',
      cursor: '#00e6ff',
    },
  },
};

