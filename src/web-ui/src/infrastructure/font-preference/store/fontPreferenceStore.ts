
import { create } from 'zustand';
import { createLogger } from '@/shared/utils/logger';
import { FontPreference, FontSizeLevel, FlowChatFontMode, DEFAULT_FONT_PREFERENCE } from '../types';
import { fontPreferenceService } from '../core/FontPreferenceService';

const log = createLogger('FontPreferenceStore');

interface FontPreferenceState {
  preference: FontPreference;
  initialized: boolean;
  loading: boolean;
  error: string | null;

  initialize: () => Promise<void>;
  setUiSize: (level: FontSizeLevel, customPx?: number) => Promise<void>;
  setFlowChatFont: (mode: FlowChatFontMode, basePx?: number) => Promise<void>;
  reset: () => Promise<void>;
}

export const useFontPreferenceStore = create<FontPreferenceState>((set, get) => ({
  preference: { ...DEFAULT_FONT_PREFERENCE },
  initialized: false,
  loading: false,
  error: null,

  initialize: async () => {
    if (get().initialized || get().loading) return;
    set({ loading: true, error: null });
    try {
      // Subscribe to service events so store stays in sync
      fontPreferenceService.on('font:after-change', (event) => {
        set({ preference: event.preference });
      });

      await fontPreferenceService.initialize();

      set({
        preference: fontPreferenceService.getPreference(),
        initialized: true,
        loading: false,
      });
    } catch (error) {
      log.error('Failed to initialize font preference', error);
      set({
        loading: false,
        error: error instanceof Error ? error.message : 'Failed to initialize font preference',
      });
    }
  },

  setUiSize: async (level: FontSizeLevel, customPx?: number) => {
    set({ error: null });
    try {
      await fontPreferenceService.setUiSize(level, customPx);
    } catch (error) {
      log.error('Failed to set UI font size', { level, customPx, error });
      set({ error: error instanceof Error ? error.message : 'Failed to set UI font size' });
    }
  },

  setFlowChatFont: async (mode: FlowChatFontMode, basePx?: number) => {
    set({ error: null });
    try {
      await fontPreferenceService.setFlowChatFont(mode, basePx);
    } catch (error) {
      log.error('Failed to set flow chat font', { mode, basePx, error });
      set({ error: error instanceof Error ? error.message : 'Failed to set flow chat font' });
    }
  },

  reset: async () => {
    set({ error: null });
    try {
      await fontPreferenceService.reset();
    } catch (error) {
      log.error('Failed to reset font preference', error);
      set({ error: error instanceof Error ? error.message : 'Failed to reset font preference' });
    }
  },
}));
