import { create } from 'zustand';
import type { ShellNavView } from './shellConfig';
import { DEFAULT_SHELL_NAV_VIEW } from './shellConfig';

interface ShellState {
  navView: ShellNavView;
  setNavView: (view: ShellNavView) => void;
}

export const useShellStore = create<ShellState>((set) => ({
  navView: DEFAULT_SHELL_NAV_VIEW,
  setNavView: (view) => set({ navView: view }),
}));
