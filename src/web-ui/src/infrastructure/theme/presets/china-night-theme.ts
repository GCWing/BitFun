 

import { ThemeConfig } from '../types';

export const bitfunChinaNightTheme: ThemeConfig = {
  
  id: 'bitfun-china-night',
  name: 'Ink Night',
  type: 'dark',
  description: 'Chinese dark theme - Starlit ink night, moonlight like water, serene and elegant',
  author: 'BitFun Team',
  version: '1.0.0',
  
  
  colors: {
    background: {
      primary: '#1a1814',          
      secondary: '#212019',        
      tertiary: '#262420',         
      quaternary: '#2d2926',       
      elevated: '#2d2926',         
      workbench: '#1a1814',        
      scene: '#1e1c17',
      tooltip: 'rgba(26, 24, 20, 0.95)',
    },
    
    text: {
      primary: '#e8e6e1',          
      secondary: '#c5c3be',        
      muted: '#928f89',            
      disabled: '#5f5d59',         
    },
    
    accent: {
      50: 'rgba(115, 165, 204, 0.04)',
      100: 'rgba(115, 165, 204, 0.08)',
      200: 'rgba(115, 165, 204, 0.15)',
      300: 'rgba(115, 165, 204, 0.25)',
      400: 'rgba(115, 165, 204, 0.4)',
      500: '#73a5cc',              
      600: '#5a8bb3',              
      700: 'rgba(90, 139, 179, 0.8)',
      800: 'rgba(90, 139, 179, 0.9)',
    },
    
    purple: {
      50: 'rgba(150, 198, 180, 0.04)',
      100: 'rgba(150, 198, 180, 0.08)',
      200: 'rgba(150, 198, 180, 0.15)',
      300: 'rgba(150, 198, 180, 0.25)',
      400: 'rgba(150, 198, 180, 0.4)',
      500: '#96c6b4',              
      600: '#7aab98',              
      700: 'rgba(122, 171, 152, 0.8)',
      800: 'rgba(122, 171, 152, 0.9)',
    },
    
    semantic: {
      success: '#6bc072',          
      successBg: 'rgba(107, 192, 114, 0.12)',
      successBorder: 'rgba(107, 192, 114, 0.3)',
      
      warning: '#f5b555',          
      warningBg: 'rgba(245, 181, 85, 0.12)',
      warningBorder: 'rgba(245, 181, 85, 0.3)',
      
      error: '#e85555',            
      errorBg: 'rgba(232, 85, 85, 0.12)',
      errorBorder: 'rgba(232, 85, 85, 0.3)',
      
      info: '#73a5cc',             
      infoBg: 'rgba(115, 165, 204, 0.12)',
      infoBorder: 'rgba(115, 165, 204, 0.3)',
      
      
      highlight: '#e6a84a',
      highlightBg: 'rgba(230, 168, 74, 0.15)',
    },
    
    border: {
      subtle: 'rgba(232, 230, 225, 0.10)',      
      base: 'rgba(232, 230, 225, 0.16)',        
      medium: 'rgba(232, 230, 225, 0.22)',      
      strong: 'rgba(232, 230, 225, 0.28)',      
      prominent: 'rgba(232, 230, 225, 0.38)',   
    },
    
    element: {
      subtle: 'rgba(115, 165, 204, 0.06)',      
      soft: 'rgba(115, 165, 204, 0.09)',        
      base: 'rgba(115, 165, 204, 0.12)',        
      medium: 'rgba(115, 165, 204, 0.16)',      
      strong: 'rgba(115, 165, 204, 0.20)',      
      elevated: 'rgba(45, 41, 38, 0.95)',       
    },
    
    git: {
      branch: 'rgb(115, 165, 204)',              
      branchBg: 'rgba(115, 165, 204, 0.12)',
      changes: 'rgb(245, 181, 85)',              
      changesBg: 'rgba(245, 181, 85, 0.12)',
      added: 'rgb(107, 192, 114)',               
      addedBg: 'rgba(107, 192, 114, 0.12)',
      deleted: 'rgb(232, 85, 85)',               
      deletedBg: 'rgba(232, 85, 85, 0.12)',
      staged: 'rgb(107, 192, 114)',              
      stagedBg: 'rgba(107, 192, 114, 0.12)',
    },
  },
  
  
  effects: {
    shadow: {
      xs: '0 1px 2px rgba(0, 0, 0, 0.5)',
      sm: '0 2px 4px rgba(0, 0, 0, 0.6)',
      base: '0 4px 8px rgba(0, 0, 0, 0.65)',
      lg: '0 8px 16px rgba(0, 0, 0, 0.7)',
      xl: '0 12px 24px rgba(0, 0, 0, 0.75)',
      '2xl': '0 16px 32px rgba(0, 0, 0, 0.8)',
    },
    
    glow: {
      blue: '0 8px 24px rgba(115, 165, 204, 0.25), 0 4px 12px rgba(115, 165, 204, 0.18), 0 2px 6px rgba(0, 0, 0, 0.3)',
      purple: '0 8px 24px rgba(150, 198, 180, 0.25), 0 4px 12px rgba(150, 198, 180, 0.18), 0 2px 6px rgba(0, 0, 0, 0.3)',
      mixed: '0 8px 24px rgba(115, 165, 204, 0.2), 0 4px 12px rgba(150, 198, 180, 0.18), 0 2px 6px rgba(0, 0, 0, 0.3)',
    },
    
    blur: {
      subtle: 'blur(4px) saturate(1.1)',
      base: 'blur(8px) saturate(1.15)',
      medium: 'blur(12px) saturate(1.2)',
      strong: 'blur(16px) saturate(1.25) brightness(1.05)',
      intense: 'blur(20px) saturate(1.3) brightness(1.08)',
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
      disabled: 0.45,
      hover: 0.75,
      focus: 0.9,
      overlay: 0.5,
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
        dot: 'rgba(115, 165, 204, 0.45)',
        dotShadow: '0 0 4px rgba(115, 165, 204, 0.2)',
        hoverBg: 'rgba(115, 165, 204, 0.12)',
        hoverColor: '#73a5cc',
        hoverBorder: 'rgba(115, 165, 204, 0.2)',
        hoverShadow: '0 2px 8px rgba(115, 165, 204, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.1)',
      },
      maximize: {
        dot: 'rgba(115, 165, 204, 0.45)',
        dotShadow: '0 0 4px rgba(115, 165, 204, 0.2)',
        hoverBg: 'rgba(115, 165, 204, 0.12)',
        hoverColor: '#73a5cc',
        hoverBorder: 'rgba(115, 165, 204, 0.2)',
        hoverShadow: '0 2px 8px rgba(115, 165, 204, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.1)',
      },
      close: {
        dot: 'rgba(232, 85, 85, 0.45)',
        dotShadow: '0 0 4px rgba(232, 85, 85, 0.2)',
        hoverBg: 'rgba(232, 85, 85, 0.12)',
        hoverColor: '#e85555',
        hoverBorder: 'rgba(232, 85, 85, 0.2)',
        hoverShadow: '0 2px 8px rgba(232, 85, 85, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.1)',
      },
      common: {
        defaultColor: 'rgba(255, 255, 255, 0.9)',
        defaultDot: 'rgba(255, 255, 255, 0.2)',
        disabledDot: 'rgba(255, 255, 255, 0.1)',
        flowGradient: 'linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.05), rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.05), transparent)',
      },
    },
    
    button: {
      
      default: {
        background: 'rgba(115, 165, 204, 0.11)',
        color: '#a29d96',
        border: 'transparent',
        shadow: 'none',
      },
      hover: {
        background: 'rgba(115, 165, 204, 0.19)',
        color: '#ccc9c4',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      active: {
        background: 'rgba(115, 165, 204, 0.15)',
        color: '#ccc9c4',
        border: 'transparent',
        shadow: 'none',
        transform: 'none',
      },
      
      
      primary: {
        default: {
          background: 'rgba(115, 165, 204, 0.24)',
          color: '#88b8d8',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(115, 165, 204, 0.34)',
          color: '#b0d5ea',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(115, 165, 204, 0.28)',
          color: '#b0d5ea',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
      
      
      ghost: {
        default: {
          background: 'transparent',
          color: '#a29d96',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: 'rgba(115, 165, 204, 0.13)',
          color: '#ccc9c4',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: 'rgba(115, 165, 204, 0.11)',
          color: '#ccc9c4',
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
      { token: 'comment', foreground: '928f89', fontStyle: 'italic' },      
      { token: 'keyword', foreground: 'e85555' },                           
      { token: 'string', foreground: '6bc072' },                            
      { token: 'number', foreground: 'f5b555' },                            
      { token: 'type', foreground: '73a5cc' },                              
      { token: 'class', foreground: '73a5cc' },                             
      { token: 'function', foreground: '96c6b4' },                          
      { token: 'variable', foreground: 'c5c3be' },                          
      { token: 'constant', foreground: 'd4a574' },                          
      { token: 'operator', foreground: 'e85555' },                          
      { token: 'tag', foreground: '73a5cc' },                               
      { token: 'attribute.name', foreground: '96c6b4' },                    
      { token: 'attribute.value', foreground: '6bc072' },                   
    ],
    colors: {
      background: '#1a1814',                      
      foreground: '#e8e6e1',                      
      lineHighlight: '#212019',                   
      selection: 'rgba(115, 165, 204, 0.25)',     
      cursor: '#73a5cc',                          
      'editor.selectionBackground': 'rgba(115, 165, 204, 0.25)',  
      'editorCursor.foreground': '#73a5cc',       
    },
  },
};

