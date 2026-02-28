/**
 * skillsSceneStore — Zustand store for the Skills scene.
 *
 * Shared between SkillsNav (left sidebar) and SkillsScene (content area)
 * so both reflect the same active view.
 */

import { create } from 'zustand';

export type SkillsView = 'market' | 'installed-all' | 'installed-user' | 'installed-project';

interface SkillsSceneState {
  activeView: SkillsView;
  setActiveView: (view: SkillsView) => void;
}

export const useSkillsSceneStore = create<SkillsSceneState>((set) => ({
  activeView: 'market',
  setActiveView: (view) => set({ activeView: view }),
}));
