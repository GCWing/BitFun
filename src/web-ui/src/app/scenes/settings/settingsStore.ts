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
  /**
   * Tabs with something the user has not seen yet, rendered as a small dot.
   *
   * The navigation stays feature-neutral: a tab owner decides when it has
   * unseen content and writes the id here, rather than SettingsNav learning to
   * query each feature.
   */
  unseenTabs: ConfigTab[];
  markTabUnseen: (tab: ConfigTab, unseen: boolean) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  activeTab: DEFAULT_SETTINGS_TAB,
  searchQuery: '',
  unseenTabs: [],

  setActiveTab: (tab) => set({ activeTab: tab }),
  setSearchQuery: (query) => set({ searchQuery: query }),
  markTabUnseen: (tab, unseen) => set((state) => {
    const has = state.unseenTabs.includes(tab);
    if (has === unseen) return state;
    return {
      unseenTabs: unseen
        ? [...state.unseenTabs, tab]
        : state.unseenTabs.filter((candidate) => candidate !== tab),
    };
  }),
}));

/** Resolve the category id for a given tab (for initial scroll / highlight) */
export function getCategoryForTab(tab: ConfigTab): string | undefined {
  return SETTINGS_CATEGORIES.find(cat => cat.tabs.some(t => t.id === tab))?.id;
}
