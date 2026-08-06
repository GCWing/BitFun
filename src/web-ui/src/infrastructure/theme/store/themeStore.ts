 

import { create } from 'zustand';
import { ThemeConfig, ThemeId, ThemeMetadata, ThemeSelectionId } from '../types';
import { themeService } from '../core/ThemeService';
import { DEFAULT_LIGHT_THEME_ID, DEFAULT_DARK_THEME_ID } from '../presets';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ThemeStore');

 
interface ThemeState {
  
  currentTheme: ThemeConfig | null;
  currentThemeId: ThemeSelectionId | null;
  themes: ThemeMetadata[];
  loading: boolean;
  error: string | null;
  systemLightId: ThemeId;
  systemDarkId: ThemeId;
  
  
  initialize: () => Promise<void>;
  setTheme: (themeId: ThemeSelectionId) => Promise<void>;
  setSystemThemeOverride: (lightId: ThemeId, darkId: ThemeId) => Promise<void>;
  refreshThemes: () => void;
  addTheme: (theme: ThemeConfig) => Promise<void>;
  removeTheme: (themeId: ThemeId) => Promise<void>;
  exportTheme: (themeId: ThemeId) => any;
}

 
export const useThemeStore = create<ThemeState>((set) => ({
  
  currentTheme: null,
  currentThemeId: null,
  themes: [],
  loading: false,
  error: null,
  systemLightId: DEFAULT_LIGHT_THEME_ID,
  systemDarkId: DEFAULT_DARK_THEME_ID,
  
  
  initialize: async () => {
    set({ loading: true, error: null });
    
    try {
      
      themeService.on('theme:after-change', () => {
        set({
          currentTheme: themeService.getCurrentTheme(),
          currentThemeId: themeService.getCurrentThemeId(),
        });
      });
      
      themeService.on('theme:register', () => {
        const themes = themeService.getThemeList();
        set({ themes });
      });
      
      themeService.on('theme:unregister', () => {
        const themes = themeService.getThemeList();
        set({ themes });
      });
      
      
      await themeService.initialize();
      await themeService.ensureUserThemesLoaded();
      
      
      const themes = themeService.getThemeList();
      
      set({
        themes,
        loading: false,
        currentTheme: themeService.getCurrentTheme(),
        currentThemeId: themeService.getCurrentThemeId(),
        systemLightId: themeService.getSystemLightId(),
        systemDarkId: themeService.getSystemDarkId(),
      });
    } catch (error) {
      log.error('Failed to initialize', error);
      set({
        loading: false,
        error: error instanceof Error ? error.message : 'Failed to initialize theme system',
      });
    }
  },
  
  
  setTheme: async (themeId: ThemeSelectionId) => {
    set({ loading: true, error: null });
    
    try {
      await themeService.applyTheme(themeId);
      
      
      
      set({ loading: false });
    } catch (error) {
      log.error('Failed to switch theme', { themeId, error });
      set({
        loading: false,
        error: error instanceof Error ? error.message : 'Failed to switch theme',
      });
    }
  },
  
  
  setSystemThemeOverride: async (lightId: ThemeId, darkId: ThemeId) => {
    try {
      await themeService.setSystemThemeOverride(lightId, darkId);
      set({
        systemLightId: lightId,
        systemDarkId: darkId,
        currentTheme: themeService.getCurrentTheme(),
        currentThemeId: themeService.getCurrentThemeId(),
      });
    } catch (error) {
      log.error('Failed to set system theme override', { lightId, darkId, error });
    }
  },
  
  
  refreshThemes: () => {
    const themes = themeService.getThemeList();
    set({ themes });
  },
  
  
  addTheme: async (theme: ThemeConfig) => {
    set({ loading: true, error: null });
    
    try {
      await themeService.registerTheme(theme);
      const themes = themeService.getThemeList();
      
      set({
        themes,
        loading: false,
      });
    } catch (error) {
      log.error('Failed to add theme', error);
      set({
        loading: false,
        error: error instanceof Error ? error.message : 'Failed to add theme',
      });
    }
  },
  
  
  removeTheme: async (themeId: ThemeId) => {
    set({ loading: true, error: null });
    
    try {
      const success = themeService.unregisterTheme(themeId);
      
      if (success) {
        const themes = themeService.getThemeList();
        set({
          themes,
          loading: false,
        });
      } else {
        set({
          loading: false,
          error: 'Failed to delete theme',
        });
      }
    } catch (error) {
      log.error('Failed to remove theme', { themeId, error });
      set({
        loading: false,
        error: error instanceof Error ? error.message : 'Failed to delete theme',
      });
    }
  },
  
  
  exportTheme: (themeId: ThemeId) => {
    return themeService.exportTheme(themeId);
  },
}));


