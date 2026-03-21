/**
 * ChatInput state store for sharing expand/collapse state across components
 */

import { create } from 'zustand';

interface ChatInputStateStore {
  /** Whether ChatInput is active (transformed from collapsed capsule to normal input) */
  isActive: boolean;
  /** Whether ChatInput is expanded (full height mode) */
  isExpanded: boolean;
  /** Measured height of the ChatInput container in pixels (0 if unknown) */
  inputHeight: number;
  
  setActive: (isActive: boolean) => void;
  setExpanded: (isExpanded: boolean) => void;
  setInputHeight: (height: number) => void;
}

export const useChatInputState = create<ChatInputStateStore>((set) => ({
  isActive: true,
  isExpanded: false,
  inputHeight: 0,
  
  setActive: (isActive) => set({ isActive }),
  setExpanded: (isExpanded) => set({ isExpanded }),
  setInputHeight: (inputHeight) => set({ inputHeight }),
}));

