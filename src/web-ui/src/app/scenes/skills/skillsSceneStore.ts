import { create } from 'zustand';

export type InstalledFilter = 'all' | 'builtin' | 'user' | 'project' | 'suite';
export type SuiteModeId = 'agentic' | 'Cowork' | 'Claw' | 'Team';

interface SkillsSceneState {
  searchDraft: string;
  marketQuery: string;
  installedFilter: InstalledFilter;
  hideDuplicates: boolean;
  isAddFormOpen: boolean;
  suiteModeId: SuiteModeId;
  setSearchDraft: (value: string) => void;
  submitMarketQuery: () => void;
  setInstalledFilter: (filter: InstalledFilter) => void;
  setHideDuplicates: (hide: boolean) => void;
  setAddFormOpen: (open: boolean) => void;
  toggleAddForm: () => void;
  setSuiteModeId: (modeId: SuiteModeId) => void;
}

export const useSkillsSceneStore = create<SkillsSceneState>((set) => ({
  searchDraft: '',
  marketQuery: '',
  installedFilter: 'all',
  hideDuplicates: false,
  isAddFormOpen: false,
  suiteModeId: 'agentic',
  setSearchDraft: (value) => set({ searchDraft: value }),
  submitMarketQuery: () => set((state) => ({ marketQuery: state.searchDraft.trim() })),
  setInstalledFilter: (filter) => set({ installedFilter: filter }),
  setHideDuplicates: (hide) => set({ hideDuplicates: hide }),
  setAddFormOpen: (open) => set({ isAddFormOpen: open }),
  toggleAddForm: () => set((state) => ({ isAddFormOpen: !state.isAddFormOpen })),
  setSuiteModeId: (modeId) => set({ suiteModeId: modeId }),
}));
