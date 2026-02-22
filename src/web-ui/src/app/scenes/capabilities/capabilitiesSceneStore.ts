/**
 * capabilitiesSceneStore — Zustand store for the Capabilities scene.
 *
 * Shared between CapabilitiesSection (left nav) and CapabilitiesScene (content area)
 * so both reflect the same active view.
 */

import { create } from 'zustand';

export type CapabilitiesView = 'sub-agents' | 'skills' | 'mcp';

interface CapabilitiesSceneState {
  activeView: CapabilitiesView;
  setActiveView: (view: CapabilitiesView) => void;
}

export const useCapabilitiesSceneStore = create<CapabilitiesSceneState>((set) => ({
  activeView: 'sub-agents',
  setActiveView: (view) => set({ activeView: view }),
}));
