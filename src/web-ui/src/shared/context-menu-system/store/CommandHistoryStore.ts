 

import { create } from 'zustand';
import { CommandHistory, CommandId } from '../types/command.types';

 
interface CommandHistoryStore {
   
  history: CommandHistory[];
   
  currentIndex: number;
   
  maxSize: number;

  // Actions
   
  addHistory: (record: CommandHistory) => void;
   
  getCommandHistory: (commandId: CommandId) => CommandHistory[];
   
  clearHistory: () => void;
   
  undo: () => CommandHistory | null;
   
  redo: () => CommandHistory | null;
   
  canUndo: () => boolean;
   
  canRedo: () => boolean;
   
  setMaxSize: (size: number) => void;
}

 
export const useCommandHistoryStore = create<CommandHistoryStore>((set, get) => ({
  history: [],
  currentIndex: -1,
  maxSize: 50,

  addHistory: (record) => {
    set((state) => {
      
      const newHistory = state.currentIndex < state.history.length - 1
        ? state.history.slice(0, state.currentIndex + 1)
        : [...state.history];

      
      newHistory.push(record);

      
      const trimmedHistory = newHistory.length > state.maxSize
        ? newHistory.slice(newHistory.length - state.maxSize)
        : newHistory;

      return {
        history: trimmedHistory,
        currentIndex: trimmedHistory.length - 1
      };
    });
  },

  getCommandHistory: (commandId) => {
    return get().history.filter(record => record.commandId === commandId);
  },

  clearHistory: () => {
    set({ history: [], currentIndex: -1 });
  },

  undo: () => {
    const state = get();
    if (state.currentIndex >= 0) {
      const record = state.history[state.currentIndex];
      set({ currentIndex: state.currentIndex - 1 });
      return record;
    }
    return null;
  },

  redo: () => {
    const state = get();
    if (state.currentIndex < state.history.length - 1) {
      const record = state.history[state.currentIndex + 1];
      set({ currentIndex: state.currentIndex + 1 });
      return record;
    }
    return null;
  },

  canUndo: () => {
    const state = get();
    return state.currentIndex >= 0 && state.history[state.currentIndex]?.canUndo;
  },

  canRedo: () => {
    const state = get();
    return state.currentIndex < state.history.length - 1;
  },

  setMaxSize: (size) => {
    set((state) => {
      const newHistory = state.history.length > size
        ? state.history.slice(state.history.length - size)
        : state.history;
      
      return {
        maxSize: size,
        history: newHistory,
        currentIndex: Math.min(state.currentIndex, newHistory.length - 1)
      };
    });
  }
}));

