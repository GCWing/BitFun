 

import { ThemeConfig } from '../types';

export const bitfunChinaStyleTheme: ThemeConfig = {
  
  id: 'bitfun-china-style',
  name: 'Ink Charm',
  type: 'light',
  description: 'Chinese style theme - Rice paper and ink, blue and vermilion, warm and elegant',
  author: 'BitFun Team',
  version: '1.0.0',
  
  
  colors: {
    background: {
      primary: '#faf8f0',          
      secondary: '#f5f3e8',        
      tertiary: '#f0ede0',         
      quaternary: '#ebe8d8',       
      elevated: '#ebe9e3',         
      workbench: '#faf8f0',        
      scene: '#fdfcf6',
      tooltip: 'rgba(250, 248, 240, 0.96)',
    },
    
    text: {
      primary: '#1a1a1a',          
      secondary: '#3d3d3d',        
      muted: '#6a6a6a',            
      disabled: '#9a9a9a',         
    },
    
    accent: {
      50: 'rgba(46, 94, 138, 0.04)',
      100: 'rgba(46, 94, 138, 0.08)',
      200: 'rgba(46, 94, 138, 0.15)',
      300: 'rgba(46, 94, 138, 0.25)',
      400: 'rgba(46, 94, 138, 0.4)',
      500: '#2e5e8a',              
      600: '#234a6d',              
      700: 'rgba(35, 74, 109, 0.8)',
      800: 'rgba(35, 74, 109, 0.9)',
    },
    
    purple: {
      50: 'rgba(126, 176, 155, 0.04)',
      100: 'rgba(126, 176, 155, 0.08)',
      200: 'rgba(126, 176, 155, 0.15)',
      300: 'rgba(126, 176, 155, 0.25)',
      400: 'rgba(126, 176, 155, 0.4)',
      500: '#7eb09b',              
      600: '#5a9078',              
      700: 'rgba(90, 144, 120, 0.8)',
      800: 'rgba(90, 144, 120, 0.9)',
    },
    
    semantic: {
      success: '#52ad5a',          
      successBg: 'rgba(82, 173, 90, 0.08)',
      successBorder: 'rgba(82, 173, 90, 0.25)',
      
      warning: '#f0a020',          
      warningBg: 'rgba(240, 160, 32, 0.08)',
      warningBorder: 'rgba(240, 160, 32, 0.25)',
      
      error: '#c8102e',            
      errorBg: 'rgba(200, 16, 46, 0.08)',
      errorBorder: 'rgba(200, 16, 46, 0.25)',
      
      info: '#2e5e8a',             
      infoBg: 'rgba(46, 94, 138, 0.08)',
      infoBorder: 'rgba(46, 94, 138, 0.25)',
    },
    
    border: {
      subtle: 'rgba(106, 92, 70, 0.12)',      
      base: 'rgba(106, 92, 70, 0.20)',        
      medium: 'rgba(106, 92, 70, 0.28)',      
      strong: 'rgba(106, 92, 70, 0.36)',      
      prominent: 'rgba(106, 92, 70, 0.48)',   
    },
    
    element: {
      subtle: 'rgba(46, 94, 138, 0.03)',      
      soft: 'rgba(46, 94, 138, 0.06)',        
      base: 'rgba(46, 94, 138, 0.10)',        
      medium: 'rgba(46, 94, 138, 0.14)',      
      strong: 'rgba(46, 94, 138, 0.18)',      
      elevated: 'rgba(255, 255, 255, 0.85)',  
    },
    
    git: {
      branch: 'rgb(46, 94, 138)',              
      branchBg: 'rgba(46, 94, 138, 0.08)',
      changes: 'rgb(240, 160, 32)',            
      changesBg: 'rgba(240, 160, 32, 0.08)',
      added: 'rgb(82, 173, 90)',               
      addedBg: 'rgba(82, 173, 90, 0.08)',
      deleted: 'rgb(200, 16, 46)',             
      deletedBg: 'rgba(200, 16, 46, 0.08)',
      staged: 'rgb(82, 173, 90)',              
      stagedBg: 'rgba(82, 173, 90, 0.08)',
    },
  },
  
  
  effects: {
    shadow: {
      xs: '0 1px 2px rgba(106, 92, 70, 0.06)',
      sm: '0 2px 4px rgba(106, 92, 70, 0.08)',
      base: '0 4px 8px rgba(106, 92, 70, 0.1)',
      lg: '0 8px 16px rgba(106, 92, 70, 0.12)',
      xl: '0 12px 24px rgba(106, 92, 70, 0.15)',
      '2xl': '0 16px 32px rgba(106, 92, 70, 0.18)',
    },
    
    glow: {
      blue: '0 8px 24px rgba(46, 94, 138, 0.18), 0 4px 12px rgba(46, 94, 138, 0.12), 0 2px 6px rgba(106, 92, 70, 0.05)',
      purple: '0 8px 24px rgba(126, 176, 155, 0.18), 0 4px 12px rgba(126, 176, 155, 0.12), 0 2px 6px rgba(106, 92, 70, 0.05)',
      mixed: '0 8px 24px rgba(46, 94, 138, 0.12), 0 4px 12px rgba(126, 176, 155, 0.10), 0 2px 6px rgba(106, 92, 70, 0.05)',
    },
    
    blur: {
      subtle: 'blur(4px) saturate(1.03)',
      base: 'blur(8px) saturate(1.05)',
      medium: 'blur(12px) saturate(1.08)',
      strong: 'blur(16px) saturate(1.1) brightness(1.02)',
      intense: 'blur(20px) saturate(1.12) brightness(1.03)',
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
      hover: 0.75,
      focus: 0.9,
      overlay: 0.35,
    },
  },
  
  
  motion: {
    duration: {
      instant: '0.1s',
      fast: '0.2s',                 
      base: '0.35s',                
      slow: '0.7s',
      lazy: '1.2s',                 
    },
    
    easing: {
      standard: 'cubic-bezier(0.4, 0, 0.2, 1)',
      decelerate: 'cubic-bezier(0, 0, 0.2, 1)',
      accelerate: 'cubic-bezier(0.4, 0, 1, 1)',
      bounce: 'cubic-bezier(0.68, -0.55, 0.265, 1.55)',
      smooth: 'cubic-bezier(0.25, 0.1, 0.25, 1)',    
    },
  },
  
  
  typography: {
    font: {
      sans: "'Noto Sans SC', 'Source Han Sans CN', 'Microsoft YaHei', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
      mono: "'Source Han Mono CN', 'Noto Sans Mono CJK SC', 'FiraCode', 'JetBrains Mono', 'Consolas', monospace",
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
      tight: 1.3,
      base: 1.6,                    
      relaxed: 1.8,
    },
  },
  
  
  components: {
    
    windowControls: {
      minimize: {
        dot: 'rgba(46, 94, 138, 0.45)',
        dotShadow: '0 0 4px rgba(46, 94, 138, 0.2)',
        hoverBg: 'rgba(46, 94, 138, 0.12)',
        hoverColor: '#2e5e8a',
        hoverBorder: 'rgba(46, 94, 138, 0.2)',
        hoverShadow: '0 2px 8px rgba(46, 94, 138, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.5)',
      },
      maximize: {
        dot: 'rgba(46, 94, 138, 0.45)',
        dotShadow: '0 0 4px rgba(46, 94, 138, 0.2)',
        hoverBg: 'rgba(46, 94, 138, 0.12)',
        hoverColor: '#2e5e8a',
        hoverBorder: 'rgba(46, 94, 138, 0.2)',
        hoverShadow: '0 2px 8px rgba(46, 94, 138, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.5)',
      },
      close: {
        dot: 'rgba(200, 16, 46, 0.45)',
        dotShadow: '0 0 4px rgba(200, 16, 46, 0.2)',
        hoverBg: 'rgba(200, 16, 46, 0.12)',
        hoverColor: '#c8102e',
        hoverBorder: 'rgba(200, 16, 46, 0.2)',
        hoverShadow: '0 2px 8px rgba(200, 16, 46, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.5)',
      },
      common: {
        defaultColor: 'rgba(26, 26, 26, 0.9)',
        defaultDot: 'rgba(106, 92, 70, 0.2)',
        disabledDot: 'rgba(106, 92, 70, 0.1)',
        flowGradient: 'linear-gradient(90deg, transparent, rgba(106, 92, 70, 0.05), rgba(106, 92, 70, 0.08), rgba(106, 92, 70, 0.05), transparent)',
      },
    },
    
    button: {
      
      default: {
        background: 'rgba(46, 94, 138, 0.09)',
        color: '#5a5a5a',
        border: 'transparent',
        shadow: 'none',
      },
      hover: {
        background: 'rgba(46, 94, 138, 0.16)',
        color: '#3a3a3a',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      active: {
        background: 'rgba(46, 94, 138, 0.12)',
        color: '#3a3a3a',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      
      
      primary: {
        default: {
          background: 'rgba(46, 94, 138, 0.20)',
          color: '#4a7aa5',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(46, 94, 138, 0.30)',
          color: '#3d6689',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(46, 94, 138, 0.24)',
          color: '#3d6689',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
      
      
      ghost: {
        default: {
          background: 'transparent',
          color: '#5a5a5a',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(46, 94, 138, 0.11)',
          color: '#3a3a3a',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(46, 94, 138, 0.08)',
          color: '#3a3a3a',
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
      { token: 'comment', foreground: '6a6a6a', fontStyle: 'italic' },      
      { token: 'keyword', foreground: 'c8102e' },                           
      { token: 'string', foreground: '52ad5a' },                            
      { token: 'number', foreground: 'f0a020' },                            
      { token: 'type', foreground: '2e5e8a' },                              
      { token: 'class', foreground: '2e5e8a' },                             
      { token: 'function', foreground: '7eb09b' },                          
      { token: 'variable', foreground: '3d3d3d' },                          
      { token: 'constant', foreground: 'a0522d' },                          
      { token: 'operator', foreground: 'c8102e' },                          
      { token: 'tag', foreground: '2e5e8a' },                               
      { token: 'attribute.name', foreground: '7eb09b' },                    
      { token: 'attribute.value', foreground: '52ad5a' },                   
    ],
    colors: {
      background: '#faf8f0',                      
      foreground: '#1a1a1a',                      
      lineHighlight: '#f5f3e8',                   
      selection: 'rgba(46, 94, 138, 0.28)',       
      cursor: '#2e5e8a',                          
      
      'editor.selectionBackground': 'rgba(46, 94, 138, 0.28)',   
      'editor.selectionForeground': '#1a1a1a',                   
      'editor.inactiveSelectionBackground': 'rgba(46, 94, 138, 0.18)',  
      'editor.selectionHighlightBackground': 'rgba(46, 94, 138, 0.20)',  
      'editor.selectionHighlightBorder': 'rgba(46, 94, 138, 0.35)',      
      'editorCursor.foreground': '#2e5e8a',       
      'editor.wordHighlightBackground': 'rgba(46, 94, 138, 0.12)',  
      'editor.wordHighlightStrongBackground': 'rgba(46, 94, 138, 0.22)',  
    },
  },
};

