/** ChatInput layout measurements shared with the transcript viewport. */

import { create } from 'zustand';

interface ChatInputStateStore {
  /** Measured height of the ChatInput container in pixels (0 if unknown) */
  inputHeight: number;
  setInputHeight: (height: number) => void;
}

export const useChatInputState = create<ChatInputStateStore>((set) => ({
  inputHeight: 0,
  setInputHeight: (inputHeight) => set({ inputHeight }),
}));

