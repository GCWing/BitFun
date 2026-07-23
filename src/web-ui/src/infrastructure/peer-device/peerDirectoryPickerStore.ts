/**
 * Imperative peer-aware workspace directory picker store.
 * Local mode uses the native dialog; Peer Mode opens an in-app browser on A
 * that lists directories via HostInvoke on B.
 */

import { create } from 'zustand';

export interface PeerDirectoryPickerOptions {
  title: string;
  defaultPath?: string;
}

interface PeerDirectoryPickerState {
  isOpen: boolean;
  title: string;
  defaultPath?: string;
  resolve: ((path: string | null) => void) | null;
  show: (options: PeerDirectoryPickerOptions) => Promise<string | null>;
  select: (path: string) => void;
  cancel: () => void;
}

export const usePeerDirectoryPickerStore = create<PeerDirectoryPickerState>((set, get) => ({
  isOpen: false,
  title: '',
  defaultPath: undefined,
  resolve: null,

  show: (options) => {
    const previous = get().resolve;
    if (previous) {
      previous(null);
    }
    return new Promise<string | null>((resolve) => {
      set({
        isOpen: true,
        title: options.title,
        defaultPath: options.defaultPath,
        resolve,
      });
    });
  },

  select: (path) => {
    const { resolve } = get();
    set({ isOpen: false, resolve: null, defaultPath: undefined });
    resolve?.(path);
  },

  cancel: () => {
    const { resolve } = get();
    set({ isOpen: false, resolve: null, defaultPath: undefined });
    resolve?.(null);
  },
}));
