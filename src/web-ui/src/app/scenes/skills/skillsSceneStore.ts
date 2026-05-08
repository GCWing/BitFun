import { create } from 'zustand';

export type InstalledFilter = 'all' | 'user' | 'project';

interface SkillsSceneState {
  searchDraft: string;
  marketQuery: string;
  installedFilter: InstalledFilter;
  hideDuplicates: boolean;
  isAddFormOpen: boolean;
  setSearchDraft: (value: string) => void;
  submitMarketQuery: () => void;
  setInstalledFilter: (filter: InstalledFilter) => void;
  setHideDuplicates: (hide: boolean) => void;
  setAddFormOpen: (open: boolean) => void;
  toggleAddForm: () => void;
}

export const useSkillsSceneStore = create<SkillsSceneState>((set) => ({
  searchDraft: '',
  marketQuery: '',
  installedFilter: 'all',
  hideDuplicates: false,
  isAddFormOpen: false,
  setSearchDraft: (value) => set({ searchDraft: value }),
  submitMarketQuery: () => set((state) => ({ marketQuery: state.searchDraft.trim() })),
  setInstalledFilter: (filter) => set({ installedFilter: filter }),
  setHideDuplicates: (hide) => set({ hideDuplicates: hide }),
  setAddFormOpen: (open) => set({ isAddFormOpen: open }),
  toggleAddForm: () => set((state) => ({ isAddFormOpen: !state.isAddFormOpen })),
}));
