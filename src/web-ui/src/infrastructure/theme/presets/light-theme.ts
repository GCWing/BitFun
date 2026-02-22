 

import { ThemeConfig } from '../types';

export const bitfunLightTheme: ThemeConfig = {
  
  id: 'bitfun-light',
  name: 'Light',
  type: 'light',
  description: 'Light theme - Elegant and refined, soft blue-gray tones',
  author: 'BitFun Team',
  version: '2.2.0',
  
  
  colors: {
    background: {
      primary: '#f7f8fa',          
      secondary: '#ffffff',        
      tertiary: '#f3f5f8',         
      quaternary: '#ebeef3',       
      elevated: '#ffffff',         
      workbench: '#f7f8fa',        
      scene: '#ffffff',
      tooltip: 'rgba(255, 255, 255, 0.98)',
    },
    
    text: {
      primary: '#1e293b',          
      secondary: '#3d4f66',        
      muted: '#64748b',            
      disabled: '#94a3b8',         
    },
    
    
    accent: {
      50: 'rgba(71, 102, 143, 0.04)',
      100: 'rgba(71, 102, 143, 0.08)',
      200: 'rgba(71, 102, 143, 0.14)',
      300: 'rgba(71, 102, 143, 0.22)',
      400: 'rgba(71, 102, 143, 0.36)',
      500: '#5a7bb2',              
      600: '#4a6694',              
      700: 'rgba(74, 102, 148, 0.8)',
      800: 'rgba(74, 102, 148, 0.9)',
    },
    
    
    purple: {
      50: 'rgba(107, 90, 137, 0.04)',
      100: 'rgba(107, 90, 137, 0.08)',
      200: 'rgba(107, 90, 137, 0.14)',
      300: 'rgba(107, 90, 137, 0.22)',
      400: 'rgba(107, 90, 137, 0.36)',
      500: '#7c6b99',              
      600: '#655680',              
      700: 'rgba(101, 86, 128, 0.8)',
      800: 'rgba(101, 86, 128, 0.9)',
    },
    
    
    semantic: {
      success: '#5b9a6f',          
      successBg: 'rgba(91, 154, 111, 0.08)',
      successBorder: 'rgba(91, 154, 111, 0.25)',
      
      warning: '#c08c42',          
      warningBg: 'rgba(192, 140, 66, 0.08)',
      warningBorder: 'rgba(192, 140, 66, 0.25)',
      
      error: '#c26565',            
      errorBg: 'rgba(194, 101, 101, 0.08)',
      errorBorder: 'rgba(194, 101, 101, 0.25)',
      
      info: '#5a7bb2',             
      infoBg: 'rgba(90, 123, 178, 0.08)',
      infoBorder: 'rgba(90, 123, 178, 0.25)',
      
      
      highlight: '#b8863a',
      highlightBg: 'rgba(184, 134, 58, 0.12)',
    },
    
    
    border: {
      subtle: 'rgba(100, 116, 139, 0.15)',     
      base: 'rgba(100, 116, 139, 0.22)',       
      medium: 'rgba(100, 116, 139, 0.32)',     
      strong: 'rgba(100, 116, 139, 0.42)',     
      prominent: 'rgba(100, 116, 139, 0.52)',  
    },
    
    
    element: {
      subtle: 'rgba(71, 102, 143, 0.05)',      
      soft: 'rgba(71, 102, 143, 0.08)',        
      base: 'rgba(71, 102, 143, 0.11)',        
      medium: 'rgba(71, 102, 143, 0.15)',      
      strong: 'rgba(71, 102, 143, 0.20)',      
      elevated: 'rgba(255, 255, 255, 0.92)',   
    },
    
    
    git: {
      branch: 'rgb(90, 123, 178)',             
      branchBg: 'rgba(90, 123, 178, 0.08)',
      changes: 'rgb(192, 140, 66)',            
      changesBg: 'rgba(192, 140, 66, 0.08)',
      added: 'rgb(91, 154, 111)',              
      addedBg: 'rgba(91, 154, 111, 0.08)',
      deleted: 'rgb(194, 101, 101)',           
      deletedBg: 'rgba(194, 101, 101, 0.08)',
      staged: 'rgb(91, 154, 111)',             
      stagedBg: 'rgba(91, 154, 111, 0.08)',
    },
  },
  
  
  effects: {
    shadow: {
      
      xs: '0 1px 2px rgba(71, 85, 105, 0.06)',
      sm: '0 2px 4px rgba(71, 85, 105, 0.08)',
      base: '0 4px 8px rgba(71, 85, 105, 0.10)',
      lg: '0 8px 16px rgba(71, 85, 105, 0.12)',
      xl: '0 12px 24px rgba(71, 85, 105, 0.14)',
      '2xl': '0 16px 32px rgba(71, 85, 105, 0.16)',
    },
    
    
    glow: {
      blue: '0 8px 24px rgba(90, 123, 178, 0.15), 0 4px 12px rgba(90, 123, 178, 0.10), 0 2px 6px rgba(71, 85, 105, 0.05)',
      purple: '0 8px 24px rgba(124, 107, 153, 0.15), 0 4px 12px rgba(124, 107, 153, 0.10), 0 2px 6px rgba(71, 85, 105, 0.05)',
      mixed: '0 8px 24px rgba(90, 123, 178, 0.12), 0 4px 12px rgba(124, 107, 153, 0.08), 0 2px 6px rgba(71, 85, 105, 0.05)',
    },
    
    blur: {
      subtle: 'blur(4px) saturate(1.02)',
      base: 'blur(8px) saturate(1.05)',
      medium: 'blur(12px) saturate(1.08)',
      strong: 'blur(16px) saturate(1.10) brightness(1.02)',
      intense: 'blur(20px) saturate(1.12) brightness(1.03)',
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
      disabled: 0.55,
      hover: 0.75,
      focus: 0.9,
      overlay: 0.35,
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
        dot: 'rgba(90, 123, 178, 0.55)',
        dotShadow: '0 0 4px rgba(90, 123, 178, 0.20)',
        hoverBg: 'rgba(90, 123, 178, 0.14)',
        hoverColor: '#4a6694',
        hoverBorder: 'rgba(90, 123, 178, 0.25)',
        hoverShadow: '0 2px 8px rgba(90, 123, 178, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.6)',
      },
      maximize: {
        dot: 'rgba(90, 123, 178, 0.55)',
        dotShadow: '0 0 4px rgba(90, 123, 178, 0.20)',
        hoverBg: 'rgba(90, 123, 178, 0.14)',
        hoverColor: '#4a6694',
        hoverBorder: 'rgba(90, 123, 178, 0.25)',
        hoverShadow: '0 2px 8px rgba(90, 123, 178, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.6)',
      },
      close: {
        dot: 'rgba(194, 101, 101, 0.55)',
        dotShadow: '0 0 4px rgba(194, 101, 101, 0.20)',
        hoverBg: 'rgba(194, 101, 101, 0.14)',
        hoverColor: '#a85555',
        hoverBorder: 'rgba(194, 101, 101, 0.25)',
        hoverShadow: '0 2px 8px rgba(194, 101, 101, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.6)',
      },
      common: {
        defaultColor: 'rgba(30, 41, 59, 0.95)',
        defaultDot: 'rgba(100, 116, 139, 0.28)',
        disabledDot: 'rgba(100, 116, 139, 0.15)',
        flowGradient: 'linear-gradient(90deg, transparent, rgba(100, 116, 139, 0.06), rgba(100, 116, 139, 0.10), rgba(100, 116, 139, 0.06), transparent)',
      },
    },
    
    button: {
      
      default: {
        background: 'rgba(71, 102, 143, 0.10)',
        color: '#475569',
        border: 'transparent',
        shadow: 'none',
      },
      hover: {
        background: 'rgba(71, 102, 143, 0.16)',
        color: '#3d4f66',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      active: {
        background: 'rgba(71, 102, 143, 0.12)',
        color: '#3d4f66',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      
      
      primary: {
        default: {
          background: 'rgba(90, 123, 178, 0.18)',
          color: '#4a6694',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(90, 123, 178, 0.28)',
          color: '#3a5478',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(90, 123, 178, 0.22)',
          color: '#3a5478',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
      
      
      ghost: {
        default: {
          background: 'transparent',
          color: '#475569',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(71, 102, 143, 0.12)',
          color: '#3d4f66',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(71, 102, 143, 0.08)',
          color: '#3d4f66',
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
      { token: 'comment', foreground: '94a3b8', fontStyle: 'italic' },      
      { token: 'keyword', foreground: '6b5a89' },                           
      { token: 'string', foreground: '5b9a6f' },                            
      { token: 'number', foreground: 'b8863a' },                            
      { token: 'type', foreground: '5a7bb2' },                              
      { token: 'class', foreground: '5a7bb2' },                             
      { token: 'function', foreground: '7c6b99' },                          
      { token: 'variable', foreground: '475569' },                          
      { token: 'constant', foreground: 'c08c42' },                          
      { token: 'operator', foreground: '6b5a89' },                          
      { token: 'tag', foreground: '5a7bb2' },                               
      { token: 'attribute.name', foreground: '7c6b99' },                    
      { token: 'attribute.value', foreground: '5b9a6f' },                   
    ],
    colors: {
      background: '#f7f8fa',                      
      foreground: '#1e293b',                      
      lineHighlight: '#f0f4f8',                   
      selection: 'rgba(90, 123, 178, 0.30)',      
      cursor: '#5a7bb2',                          
      
      
      'editor.selectionBackground': 'rgba(90, 123, 178, 0.30)',  
      'editor.selectionForeground': '#1e293b',     
      'editor.inactiveSelectionBackground': 'rgba(90, 123, 178, 0.20)',  
      'editor.selectionHighlightBackground': 'rgba(90, 123, 178, 0.22)',  
      'editor.selectionHighlightBorder': 'rgba(90, 123, 178, 0.40)',      
      'editorCursor.foreground': '#5a7bb2',       
      
      'editor.wordHighlightBackground': 'rgba(90, 123, 178, 0.15)',  
      'editor.wordHighlightStrongBackground': 'rgba(90, 123, 178, 0.25)',  
    },
  },
};





