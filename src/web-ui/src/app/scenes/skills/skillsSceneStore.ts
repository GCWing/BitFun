import { create } from 'zustand';

export type InstalledFilter = 'all' | 'builtin' | 'user' | 'project' | `source:${string}`;
export type SkillsView = InstalledFilter | 'groups';

interface SkillsSceneState {
  searchDraft: string;
  marketQuery: string;
  installedView: SkillsView;
  hideDuplicates: boolean;
  isAddFormOpen: boolean;
  setSearchDraft: (value: string) => void;
  submitMarketQuery: () => void;
  setInstalledView: (view: SkillsView) => void;
  setHideDuplicates: (hide: boolean) => void;
  setAddFormOpen: (open: boolean) => void;
  toggleAddForm: () => void;
}

export const useSkillsSceneStore = create<SkillsSceneState>((set) => ({
  searchDraft: '',
  marketQuery: '',
  installedView: 'all',
  hideDuplicates: false,
  isAddFormOpen: false,
  setSearchDraft: (value) => set({ searchDraft: value }),
  submitMarketQuery: () => set((state) => ({ marketQuery: state.searchDraft.trim() })),
  setInstalledView: (view) => set({ installedView: view }),
  setHideDuplicates: (hide) => set({ hideDuplicates: hide }),
  setAddFormOpen: (open) => set({ isAddFormOpen: open }),
  toggleAddForm: () => set((state) => ({ isAddFormOpen: !state.isAddFormOpen })),
}));
