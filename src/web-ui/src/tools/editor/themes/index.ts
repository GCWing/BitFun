/**
 * Monaco theme registry - manages theme registration and prevents duplicates
 */

import * as monaco from 'monaco-editor';
import { BitFunDarkTheme, BitFunDarkThemeMetadata } from './bitfun-dark.theme';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ThemeRegistry');

const registeredThemes = new Set<string>();

/**
 * Register a theme safely (prevents duplicate registration).
 * Returns false if the theme is already registered.
 */
export function registerTheme(
  id: string, 
  theme: monaco.editor.IStandaloneThemeData
): boolean {
  if (registeredThemes.has(id)) {
    return false;
  }
  
  try {
    monaco.editor.defineTheme(id, theme);
    registeredThemes.add(id);
    return true;
  } catch (error) {
    log.error('Failed to register theme', { themeId: id, error });
    return false;
  }
}

/**
 * Force register a theme (for theme updates or hot reload).
 */
export function forceRegisterTheme(
  id: string, 
  theme: monaco.editor.IStandaloneThemeData
): void {
  try {
    monaco.editor.defineTheme(id, theme);
    registeredThemes.add(id);
  } catch (error) {
    log.error('Failed to force register theme', { themeId: id, error });
  }
}

export function isThemeRegistered(id: string): boolean {
  return registeredThemes.has(id);
}

export function getRegisteredThemes(): string[] {
  return Array.from(registeredThemes);
}

export function initializeBuiltinThemes(): boolean {
  return registerTheme(BitFunDarkThemeMetadata.id, BitFunDarkTheme);
}

export function ensureDefaultTheme(): void {
  if (!registeredThemes.has(BitFunDarkThemeMetadata.id)) {
    registerTheme(BitFunDarkThemeMetadata.id, BitFunDarkTheme);
  }
}

export function isThemesInitialized(): boolean {
  return registeredThemes.has(BitFunDarkThemeMetadata.id);
}

export function applyTheme(themeId: string): void {
  try {
    monaco.editor.setTheme(themeId);
  } catch (error) {
    log.error('Failed to apply theme', { themeId, error });
  }
}

export function getDefaultThemeId(): string {
  return BitFunDarkThemeMetadata.id;
}

export { BitFunDarkTheme, BitFunDarkThemeMetadata };
