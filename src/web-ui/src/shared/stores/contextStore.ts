

import { create } from 'zustand';
import { devtools, persist, StorageValue } from 'zustand/middleware';
import { ContextItem, ValidationResult } from '../types/context';
import { createLogger } from '@/shared/utils/logger';
import { storage } from '@/shared/utils/storageAdapter';

const log = createLogger('ContextStore');



interface ContextState {

  contexts: ContextItem[];


  validationStates: Map<string, ValidationResult>;


  validatingIds: Set<string>;

  // Actions
  addContext: (item: ContextItem) => void;
  removeContext: (id: string) => void;
  clearContexts: () => void;
  updateValidation: (id: string, result: ValidationResult) => void;
  setValidating: (id: string, validating: boolean) => void;
  reorderContexts: (startIndex: number, endIndex: number) => void;
  updateContext: (id: string, updates: Partial<ContextItem>) => void;
}



export const useContextStore = create<ContextState>()(
  devtools(
    persist(
      (set, _get) => ({

        contexts: [],
        validationStates: new Map(),
        validatingIds: new Set(),


        addContext: (item: ContextItem) => {
          set((state) => {

            if (state.contexts.some(c => c.id === item.id)) {
              log.warn('Context already exists', { id: item.id });
              return state;
            }

            return {
              contexts: [...state.contexts, item]
            };
          }, false, 'addContext');
        },


        removeContext: (id: string) => {
          set((state) => {
            const newValidationStates = new Map(state.validationStates);
            newValidationStates.delete(id);

            const newValidatingIds = new Set(state.validatingIds);
            newValidatingIds.delete(id);

            return {
              contexts: state.contexts.filter(c => c.id !== id),
              validationStates: newValidationStates,
              validatingIds: newValidatingIds
            };
          }, false, 'removeContext');
        },


        clearContexts: () => {
          set({
            contexts: [],
            validationStates: new Map(),
            validatingIds: new Set()
          }, false, 'clearContexts');
        },


        updateValidation: (id: string, result: ValidationResult) => {
          set((state) => {
            const newValidationStates = new Map(state.validationStates);
            newValidationStates.set(id, result);

            const newValidatingIds = new Set(state.validatingIds);
            newValidatingIds.delete(id);

            return {
              validationStates: newValidationStates,
              validatingIds: newValidatingIds
            };
          }, false, 'updateValidation');
        },


        setValidating: (id: string, validating: boolean) => {
          set((state) => {
            const newValidatingIds = new Set(state.validatingIds);
            if (validating) {
              newValidatingIds.add(id);
            } else {
              newValidatingIds.delete(id);
            }

            return { validatingIds: newValidatingIds };
          }, false, 'setValidating');
        },


        reorderContexts: (startIndex: number, endIndex: number) => {
          set((state) => {
            const newContexts = [...state.contexts];
            const [removed] = newContexts.splice(startIndex, 1);
            newContexts.splice(endIndex, 0, removed);

            return { contexts: newContexts };
          }, false, 'reorderContexts');
        },


        updateContext: (id: string, updates: Partial<ContextItem>) => {
          set((state) => {
            const contexts = state.contexts.map(c =>
              c.id === id ? { ...c, ...updates } as ContextItem : c
            );

            return { contexts };
          }, false, 'updateContext');
        }
      }),
      {
        name: 'bitfun-context-storage',
        storage: {
          getItem(name: string) {
            try {
              return storage.getItem(name);
            } catch (e) {
              console.error(`Failed to get item from storage ${e}`);
            }
          },
          setItem(name: string, value: StorageValue<ContextState>) {
            try {
              return storage.setItem(name, JSON.stringify(value));
            } catch (e) {
              console.error(`Failed to set item from storage ${e}`);
            }
          },
          removeItem(name: string) {
            try {
              return storage.removeItem(name);
            } catch (e) {
              console.error(`Failed to remove item from storage ${e}`);
            }
          }
        },
        serialize: (state: any) => {
          return JSON.stringify({
            ...state.state,
            validationStates: Array.from(state.state.validationStates.entries()),
            validatingIds: Array.from(state.state.validatingIds)
          });
        },

        deserialize: (str: string) => {
          const parsed = JSON.parse(str);
          return {
            ...parsed,
            state: {
              ...parsed.state,
              validationStates: new Map(parsed.state.validationStates),
              validatingIds: new Set(parsed.state.validatingIds)
            }
          };
        },

        partialize: (state: any) => ({
          contexts: state.contexts.filter((ctx: any) => ctx.type !== 'image')
        })
      } as any
    ),
    {
      name: 'ContextStore',
      enabled: process.env.NODE_ENV === 'development'
    }
  )
);



export const selectContexts = (state: ContextState) => state.contexts;
export const selectContextCount = (state: ContextState) => state.contexts.length;
export const selectContextById = (id: string) => (state: ContextState) =>
  state.contexts.find(c => c.id === id);
export const selectValidationState = (id: string) => (state: ContextState) =>
  state.validationStates.get(id);
export const selectIsValidating = (id: string) => (state: ContextState) =>
  state.validatingIds.has(id);
export const selectHasInvalidContexts = (state: ContextState) =>
  Array.from(state.validationStates.values()).some(v => !v.valid);




export const cleanupImageContextsFromStorage = () => {
  try {
    const storageKey = 'bitfun-context-storage';
    const stored = storage.getItem(storageKey);

    if (stored) {
      const parsed = JSON.parse(stored);

      if (parsed.state && Array.isArray(parsed.state.contexts)) {
        const imageCount = parsed.state.contexts.filter((ctx: any) => ctx.type === 'image').length;

        if (imageCount > 0) {

          parsed.state.contexts = parsed.state.contexts.filter((ctx: any) => ctx.type !== 'image');


          storage.setItem(storageKey, JSON.stringify(parsed));
        }
      }
    }
  } catch (error) {
    log.warn('Failed to cleanup image contexts', error);
  }
};


cleanupImageContextsFromStorage();
