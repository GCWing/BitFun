import { create } from 'zustand';
import type { InteractionMotion } from '@/shared/utils/motionPreference';
import { DEFAULT_SETTINGS_PAGE_ID } from './settingsRegistry';
import type {
  SettingsDestination,
  SettingsPageId,
  SettingsViewId,
} from './settingsTypes';

interface SettingsState {
  activePageId: SettingsPageId;
  activeViewId: SettingsViewId | null;
  navigationRequestId: number;
  pageTransitionTarget: SettingsPageId | null;
  pageTransitionMotion: InteractionMotion;
  pageTransitionSequence: number;
  openDestination: (destination: SettingsDestination, motion?: InteractionMotion) => void;
  openPage: (pageId: SettingsPageId, motion?: InteractionMotion) => void;
  setActiveView: (viewId: SettingsViewId) => void;
  searchQuery: string;
  setSearchQuery: (query: string) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  activePageId: DEFAULT_SETTINGS_PAGE_ID,
  activeViewId: null,
  navigationRequestId: 0,
  pageTransitionTarget: null,
  pageTransitionMotion: 'instant',
  pageTransitionSequence: 0,
  searchQuery: '',

  openDestination: (destination, motion = 'instant') => set((state) => ({
    activePageId: destination.pageId,
    activeViewId: destination.viewId ?? null,
    navigationRequestId: state.navigationRequestId + 1,
    pageTransitionTarget: destination.pageId,
    pageTransitionMotion: motion,
    pageTransitionSequence: state.pageTransitionSequence + 1,
  })),
  openPage: (pageId, motion = 'instant') => set((state) => ({
    activePageId: pageId,
    activeViewId: null,
    navigationRequestId: state.navigationRequestId + 1,
    pageTransitionTarget: pageId,
    pageTransitionMotion: motion,
    pageTransitionSequence: state.pageTransitionSequence + 1,
  })),
  setActiveView: (viewId) => set((state) => ({
    activeViewId: viewId,
    navigationRequestId: state.navigationRequestId + 1,
  })),
  setSearchQuery: (query) => set({ searchQuery: query }),
}));
