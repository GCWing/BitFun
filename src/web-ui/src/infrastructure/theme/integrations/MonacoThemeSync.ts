 

import * as monaco from 'monaco-editor';
import { ThemeConfig } from '../types';
import { BitFunDarkTheme } from '@/tools/editor/themes/bitfun-dark.theme';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('MonacoThemeSync');


const SEMANTIC_HIGHLIGHTING_RULES = BitFunDarkTheme.rules;

 
function convertToHexColor(color: string): string {
  if (!color) return color;
  
  
  if (color.startsWith('#')) {
    return color;
  }
  
  
  const rgbaMatch = color.match(/rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+))?\s*\)/i);
  if (rgbaMatch) {
    const r = parseInt(rgbaMatch[1], 10);
    const g = parseInt(rgbaMatch[2], 10);
    const b = parseInt(rgbaMatch[3], 10);
    const a = rgbaMatch[4] !== undefined ? parseFloat(rgbaMatch[4]) : 1;
    
    
    const toHex = (n: number) => n.toString(16).padStart(2, '0');
    const alphaHex = Math.round(a * 255).toString(16).padStart(2, '0');
    
    return `#${toHex(r)}${toHex(g)}${toHex(b)}${alphaHex}`;
  }
  
  
  return color;
}

 
function convertColorsToHex(colors: Record<string, string>): Record<string, string> {
  const result: Record<string, string> = {};
  for (const [key, value] of Object.entries(colors)) {
    result[key] = convertToHexColor(value);
  }
  return result;
}

 
export class MonacoThemeSync {
  private initialized = false;
  private currentThemeId: string | null = null;
  
   
  async initialize(): Promise<void> {
    if (this.initialized) {
      return;
    }
    
    
    try {
      monaco.editor.defineTheme('bitfun-dark', BitFunDarkTheme);
      log.debug('BitFun Dark theme registered');
      this.initialized = true;
    } catch (error) {
      log.warn('Monaco Editor not loaded yet, will retry later', error);
    }
  }
  
   
  syncTheme(theme: ThemeConfig): void {
    try {
      let targetThemeId: string;
      
      
      if (theme.monaco) {
        const monacoTheme = this.convertToMonacoTheme(theme);
        
        monaco.editor.defineTheme(theme.id, monacoTheme);
        targetThemeId = theme.id;
        log.debug('Custom theme registered', { themeId: theme.id, themeName: theme.name });
      } else {
        
        if (theme.type === 'dark') {
          targetThemeId = 'bitfun-dark';
        } else {
          
          targetThemeId = 'bitfun-light';
          
          monaco.editor.defineTheme('bitfun-light', {
            base: 'vs',
            inherit: true,
            rules: SEMANTIC_HIGHLIGHTING_RULES,
            colors: convertColorsToHex({
              'focusBorder': '#00000000',
              'contrastBorder': '#00000000',
              'diffEditor.insertedTextBorder': '#00000000',
              'diffEditor.removedTextBorder': '#00000000',
              
              'editor.selectionBackground': 'rgba(90, 123, 178, 0.30)',
              'editor.selectionForeground': '#1e293b',
              'editor.inactiveSelectionBackground': 'rgba(90, 123, 178, 0.20)',
              'editor.selectionHighlightBackground': 'rgba(90, 123, 178, 0.22)',
              'editor.selectionHighlightBorder': 'rgba(90, 123, 178, 0.40)',
              'editor.wordHighlightBackground': 'rgba(90, 123, 178, 0.15)',
              'editor.wordHighlightStrongBackground': 'rgba(90, 123, 178, 0.25)',
            })
          });
        }
        log.debug('Using builtin theme', { themeId: targetThemeId });
      }
      
      
      monaco.editor.setTheme(targetThemeId);
      
      
      
      const editors = monaco.editor.getEditors();
      if (editors && editors.length > 0) {
        log.debug('Refreshing editor instances', { count: editors.length });
        editors.forEach((editor, index) => {
          try {
            
            editor.updateOptions({});
          } catch (err) {
            log.warn('Failed to refresh editor instance', { index, error: err });
          }
        });
      }
      
      this.currentThemeId = targetThemeId;
      log.info('Theme switched successfully', { themeName: theme.name, themeId: targetThemeId });
      
      
      window.dispatchEvent(new CustomEvent('monaco-theme-changed', {
        detail: { themeId: targetThemeId, theme }
      }));
    } catch (error) {
      log.error('Failed to sync theme', error);
    }
  }
  
   
  getCurrentThemeId(): string | null {
    return this.currentThemeId;
  }
  
   
  private convertToMonacoTheme(theme: ThemeConfig): monaco.editor.IStandaloneThemeData {
    const { monaco: monacoConfig, colors } = theme;
    if (!monacoConfig) {
      
      
      
      return {
        base: theme.type === 'dark' ? 'vs-dark' : 'vs',
        inherit: true,
        rules: SEMANTIC_HIGHLIGHTING_RULES,
        colors: convertColorsToHex({
          'editor.background': colors.background.scene,
          'editor.foreground': colors.text.primary,
          'editor.selectionBackground': colors.accent[300], 
          'editorCursor.foreground': colors.accent[500],   
        }),
      };
    }
    
    
    
    const themeRules = monacoConfig.rules.length > 0
      ? monacoConfig.rules.map(rule => ({
          token: rule.token,
          foreground: rule.foreground,
          background: rule.background,
          fontStyle: rule.fontStyle,
        }))
      : SEMANTIC_HIGHLIGHTING_RULES;
    
    
    const themeData: monaco.editor.IStandaloneThemeData = {
      base: monacoConfig.base,
      inherit: monacoConfig.inherit,
      rules: themeRules,
      colors: this.mergeEditorColors(monacoConfig.colors, colors),
    };
    
    return themeData;
  }
  
   
  private mergeEditorColors(
    monacoColors: any,
    themeColors: ThemeConfig['colors']
  ): Record<string, string> {
    
    
    const baseColors: Record<string, string> = {
      'editor.background': themeColors.background.scene,
      'editor.foreground': themeColors.text.primary,
      'editorLineNumber.foreground': themeColors.text.muted,
      'editorCursor.foreground': themeColors.accent[500],
      
      'editor.selectionBackground': themeColors.accent[300],
      'editor.inactiveSelectionBackground': themeColors.accent[200],
      'editor.selectionHighlightBackground': themeColors.accent[200],
      'editor.selectionHighlightBorder': themeColors.accent[400],
      'editor.wordHighlightBackground': themeColors.accent[100],
      'editor.wordHighlightStrongBackground': themeColors.accent[200],
      'editor.lineHighlightBackground': themeColors.background.secondary,
      
      'focusBorder': '#00000000',
      'contrastBorder': '#00000000',
      
      'diffEditor.insertedTextBorder': '#00000000',
      'diffEditor.removedTextBorder': '#00000000',
    };
    
    
    const mappedMonacoColors: Record<string, string> = {};
    if (monacoColors) {
      if (monacoColors.foreground) {
        mappedMonacoColors['editor.foreground'] = monacoColors.foreground;
      }
      if (monacoColors.lineHighlight) {
        mappedMonacoColors['editor.lineHighlightBackground'] = monacoColors.lineHighlight;
      }
      if (monacoColors.selection) {
        mappedMonacoColors['editor.selectionBackground'] = monacoColors.selection;
        
        
        const isLightSelection = this.isLightColor(monacoColors.selection);
        if (!isLightSelection) {
          
          mappedMonacoColors['editor.selectionForeground'] = '#FFFFFF';
        }
      }
      if (monacoColors.cursor) {
        mappedMonacoColors['editorCursor.foreground'] = monacoColors.cursor;
      }
      
      
      
      Object.keys(monacoColors).forEach(key => {
        if (!['background', 'foreground', 'lineHighlight', 'selection', 'cursor'].includes(key)) {
          mappedMonacoColors[key] = monacoColors[key];
        }
      });
    }
    
    
    
    const mergedColors = {
      ...baseColors,
      ...mappedMonacoColors,
    };
    
    
    
    const hexColors = convertColorsToHex(mergedColors);
    
    return hexColors;
  }
  
   
  private isLightColor(color: string): boolean {
    
    let rgb: number[];
    
    if (color.startsWith('rgba') || color.startsWith('rgb')) {
      
      const match = color.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
      if (match) {
        rgb = [parseInt(match[1]), parseInt(match[2]), parseInt(match[3])];
      } else {
        return false;
      }
    } else if (color.startsWith('#')) {
      // #c8102e
      const hex = color.substring(1);
      const r = parseInt(hex.substring(0, 2), 16);
      const g = parseInt(hex.substring(2, 4), 16);
      const b = parseInt(hex.substring(4, 6), 16);
      rgb = [r, g, b];
    } else {
      return false;
    }
    
    
    const [r, g, b] = rgb;
    const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
    
    
    return luminance > 0.5;
  }
  
   
  registerTheme(themeId: string, theme: ThemeConfig): void {
    try {
      const monacoTheme = this.convertToMonacoTheme(theme);
      monaco.editor.defineTheme(themeId, monacoTheme);
      log.debug('Theme registered', { themeId });
    } catch (error) {
      log.error('Failed to register theme', { themeId, error });
    }
  }
}


export const monacoThemeSync = new MonacoThemeSync();





