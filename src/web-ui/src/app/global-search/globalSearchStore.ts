import { create } from 'zustand';

interface GlobalSearchOverlayState {
  open: boolean;
  initialQuery: string;
  openSearch: (initialQuery?: string) => void;
  closeSearch: () => void;
  toggleSearch: () => void;
}

export const useGlobalSearchStore = create<GlobalSearchOverlayState>((set) => ({
  open: false,
  initialQuery: '',
  openSearch: (initialQuery = '') => set({ open: true, initialQuery }),
  closeSearch: () => set({ open: false, initialQuery: '' }),
  toggleSearch: () => set((state) => ({
    open: !state.open,
    initialQuery: state.open ? '' : state.initialQuery,
  })),
}));

export const openGlobalSearch = (initialQuery = ''): void => {
  useGlobalSearchStore.getState().openSearch(initialQuery);
};
export const closeGlobalSearch = (): void => {
  useGlobalSearchStore.getState().closeSearch();
};
