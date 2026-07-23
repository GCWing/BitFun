/**
 * settingsStore — Zustand store for the Settings scene.
 *
 * Shared between SettingsNav (left sidebar) and SettingsScene (content area)
 * so both always reflect the same active tab without prop-drilling.
 */

import { create } from 'zustand';
import type { ConfigTab } from './settingsConfig';
import { DEFAULT_SETTINGS_TAB, SETTINGS_CATEGORIES } from './settingsConfig';

interface SettingsState {
  activeTab: ConfigTab;
  setActiveTab: (tab: ConfigTab) => void;
  /** Debounced from SettingsNav search input; used for filtering index. */
  searchQuery: string;
  setSearchQuery: (query: string) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  activeTab: DEFAULT_SETTINGS_TAB,
  searchQuery: '',

  setActiveTab: (tab) => set({ activeTab: tab }),
  setSearchQuery: (query) => set({ searchQuery: query }),
}));

/** Resolve the category id for a given tab (for initial scroll / highlight) */
export function getCategoryForTab(tab: ConfigTab): string | undefined {
  return SETTINGS_CATEGORIES.find(cat => cat.tabs.some(t => t.id === tab))?.id;
}
