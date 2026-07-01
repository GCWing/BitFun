 

import type * as Monaco from 'monaco-editor';
import { ThemeConfig } from '../types';
import { BitFunDarkTheme } from '@/tools/editor/themes/bitfun-dark.theme';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('MonacoThemeSync');


const SEMANTIC_HIGHLIGHTING_RULES = BitFunDarkTheme.rules;
const TRANSPARENT_MONACO_BORDER = '#00000000';
const LIGHT_MONACO_PASSIVE_SELECTION = 'rgba(15, 23, 42, 0.1)';
const TRANSPARENT_MONACO_BORDER_COLORS = {
  'focusBorder': TRANSPARENT_MONACO_BORDER,
  'contrastBorder': TRANSPARENT_MONACO_BORDER,
  'diffEditor.insertedTextBorder': TRANSPARENT_MONACO_BORDER,
  'diffEditor.removedTextBorder': TRANSPARENT_MONACO_BORDER,
} as const;

function getBitfunLightMonacoTheme(): Monaco.editor.IStandaloneThemeData {
  return {
    base: 'vs',
    inherit: true,
    rules: SEMANTIC_HIGHLIGHTING_RULES,
    colors: convertColorsToHex({
      ...TRANSPARENT_MONACO_BORDER_COLORS,

      'editor.selectionBackground': 'rgba(15, 23, 42, 0.14)',
      'editor.selectionForeground': '#1e293b',
      'editor.inactiveSelectionBackground': LIGHT_MONACO_PASSIVE_SELECTION,
      'editor.selectionHighlightBackground': LIGHT_MONACO_PASSIVE_SELECTION,
      'editor.selectionHighlightBorder': 'rgba(15, 23, 42, 0.22)',
      'editor.wordHighlightBackground': LIGHT_MONACO_PASSIVE_SELECTION,
      'editor.wordHighlightStrongBackground': LIGHT_MONACO_PASSIVE_SELECTION,
    }),
  };
}

 
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
  private monacoInstance: typeof Monaco | null = null;
  private pendingTheme: ThemeConfig | null = null;
  private pendingRegisteredThemes = new Map<string, ThemeConfig>();
  
  async initialize(): Promise<void> {
    if (this.initialized) {
      return;
    }

    this.initialized = true;
    if (this.monacoInstance) {
      this.ensureBuiltinThemes(this.monacoInstance);
    }
  }
  
  syncTheme(theme: ThemeConfig): string {
    this.pendingTheme = theme;
    const targetThemeId = this.getTargetMonacoThemeId(theme);

    if (!this.monacoInstance) {
      log.debug('Monaco runtime not loaded; theme sync deferred', { themeId: targetThemeId });
      return targetThemeId;
    }

    this.applyTheme(this.monacoInstance, theme, targetThemeId);
    return targetThemeId;
  }

  attachMonaco(monacoInstance: typeof Monaco, theme?: ThemeConfig): string {
    this.monacoInstance = monacoInstance;
    this.initialized = true;
    this.ensureBuiltinThemes(monacoInstance);
    this.flushPendingRegisteredThemes(monacoInstance);

    const activeTheme = theme ?? this.pendingTheme;
    if (!activeTheme) {
      return this.currentThemeId ?? 'bitfun-dark';
    }

    const targetThemeId = this.getTargetMonacoThemeId(activeTheme);
    this.applyTheme(monacoInstance, activeTheme, targetThemeId);
    return targetThemeId;
  }

  private ensureBuiltinThemes(monacoInstance: typeof Monaco): void {
    try {
      monacoInstance.editor.defineTheme('bitfun-dark', BitFunDarkTheme);
      monacoInstance.editor.defineTheme('bitfun-light', getBitfunLightMonacoTheme());
      log.debug('BitFun Monaco themes registered');
    } catch (error) {
      log.warn('Failed to register BitFun Monaco themes', error);
    }
  }

  private flushPendingRegisteredThemes(monacoInstance: typeof Monaco): void {
    if (this.pendingRegisteredThemes.size === 0) {
      return;
    }

    for (const [themeId, theme] of this.pendingRegisteredThemes) {
      this.defineTheme(monacoInstance, themeId, theme);
    }
    this.pendingRegisteredThemes.clear();
  }

  private applyTheme(
    monacoInstance: typeof Monaco,
    theme: ThemeConfig,
    targetThemeId: string,
  ): void {
    try {
      if (this.currentThemeId === targetThemeId) {
        return;
      }

      if (theme.monaco) {
        const monacoTheme = this.convertToMonacoTheme(theme);
        monacoInstance.editor.defineTheme(theme.id, monacoTheme);
        log.debug('Custom theme registered', { themeId: theme.id, themeName: theme.name });
      } else {
        log.debug('Using builtin theme', { themeId: targetThemeId });
      }

      monacoInstance.editor.setTheme(targetThemeId);

      const editors = monacoInstance.editor.getEditors();
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

      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('monaco-theme-changed', {
          detail: { themeId: targetThemeId, theme }
        }));
      }
    } catch (error) {
      log.error('Failed to sync theme', error);
    }
  }
  
   
  getCurrentThemeId(): string | null {
    return this.currentThemeId;
  }

  /**
   * Resolves which Monaco theme id should be active for the given app theme
   * (same rules as {@link syncTheme}).
   */
  getTargetMonacoThemeId(theme: ThemeConfig): string {
    if (theme.monaco) {
      return theme.id;
    }
    return theme.type === 'dark' ? 'bitfun-dark' : 'bitfun-light';
  }

  /**
   * Registers BitFun built-in and optional custom Monaco themes on the given Monaco instance.
   * Use from the Monaco React wrapper `beforeMount` hook so themes exist on the loader's Monaco
   * before the editor is created (avoids falling back to the default light theme).
   */
  registerThemesForEditorInstance(monacoInstance: typeof Monaco, theme: ThemeConfig): string {
    try {
      return this.attachMonaco(monacoInstance, theme);
    } catch (error) {
      log.error('registerThemesForEditorInstance failed', error);
      return 'bitfun-dark';
    }
  }
  
   
  private convertToMonacoTheme(theme: ThemeConfig): Monaco.editor.IStandaloneThemeData {
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
    
    
    const themeData: Monaco.editor.IStandaloneThemeData = {
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

      ...TRANSPARENT_MONACO_BORDER_COLORS,
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
          
          mappedMonacoColors['editor.selectionForeground'] = '#ffffff';
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
      if (!this.monacoInstance) {
        this.pendingRegisteredThemes.set(themeId, theme);
        log.debug('Monaco runtime not loaded; custom theme registration deferred', { themeId });
        return;
      }
      this.defineTheme(this.monacoInstance, themeId, theme);
    } catch (error) {
      log.error('Failed to register theme', { themeId, error });
    }
  }

  private defineTheme(monacoInstance: typeof Monaco, themeId: string, theme: ThemeConfig): void {
    const monacoTheme = this.convertToMonacoTheme(theme);
    monacoInstance.editor.defineTheme(themeId, monacoTheme);
    log.debug('Theme registered', { themeId });
  }
}


export const monacoThemeSync = new MonacoThemeSync();





