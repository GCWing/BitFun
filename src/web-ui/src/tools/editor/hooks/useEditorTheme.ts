/**
 * Editor Theme Hook
 * 
 * Responsibilities:
 * - Subscribe to ThemeManager changes
 * - Auto-update editor theme
 * @module useEditorTheme
 */

import { useState, useEffect, useCallback } from 'react';
import type * as monaco from 'monaco-editor';
import { themeManager, type ThemeChangeEvent } from '../services/ThemeManager';

interface UseEditorThemeOptions {
  /** Editor instance */
  editor?: monaco.editor.IStandaloneCodeEditor | null;
  /** Initial theme ID */
  initialTheme?: string;
  /** Theme change callback */
  onThemeChange?: (event: ThemeChangeEvent) => void;
}

interface UseEditorThemeResult {
  /** Current theme ID */
  currentThemeId: string;
  /** Set theme */
  setTheme: (themeId: string) => void;
  /** Registered theme list */
  registeredThemes: string[];
  /** Default theme ID */
  defaultThemeId: string;
}

/** Editor Theme Hook */
export function useEditorTheme(options: UseEditorThemeOptions = {}): UseEditorThemeResult {
  const { editor, initialTheme, onThemeChange } = options;
  
  const [currentThemeId, setCurrentThemeId] = useState(() => {
    return initialTheme || themeManager.getCurrentThemeId();
  });
  const [registeredThemes, setRegisteredThemes] = useState<string[]>(() => {
    return themeManager.getRegisteredThemes();
  });
  
  /** Ensure theme system is initialized */
  useEffect(() => {
    themeManager.initialize();
    setRegisteredThemes(themeManager.getRegisteredThemes());
  }, []);
  
  /** Subscribe to theme changes */
  useEffect(() => {
    const unsubscribe = themeManager.onThemeChange((event) => {
      setCurrentThemeId(event.currentThemeId);
      
      if (onThemeChange) {
        onThemeChange(event);
      }
      
      setRegisteredThemes(themeManager.getRegisteredThemes());
    });
    
    return unsubscribe;
  }, [onThemeChange]);
  
  /** Sync theme when editor is available */
  useEffect(() => {
    if (editor && currentThemeId) {
      import('monaco-editor').then((monaco) => {
        monaco.editor.setTheme(currentThemeId);
      });
    }
  }, [editor, currentThemeId]);
  
  const setTheme = useCallback((themeId: string) => {
    themeManager.setTheme(themeId);
  }, []);
  
  return {
    currentThemeId,
    setTheme,
    registeredThemes,
    defaultThemeId: themeManager.getDefaultThemeId(),
  };
}

export default useEditorTheme;
