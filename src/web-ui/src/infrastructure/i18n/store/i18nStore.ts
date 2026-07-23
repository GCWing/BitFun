 

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { LocaleId, I18nNamespace, I18nState, I18nActions } from '../types';
import { DEFAULT_LOCALE, DEFAULT_FALLBACK_LOCALE } from '../presets';

 
const initialState: I18nState = {
  currentLanguage: DEFAULT_LOCALE,
  fallbackLanguage: DEFAULT_FALLBACK_LOCALE,
  loadedNamespaces: [],
  isInitialized: false,
  isChanging: false,
  autoDetect: false,
};

/**
 * I18n Store
 */
export const useI18nStore = create<I18nState & I18nActions>()(
  persist(
    (set) => ({
      
      ...initialState,

      // Actions
      setCurrentLanguage: (locale: LocaleId) => {
        set((state) => (
          state.currentLanguage === locale
            ? state
            : { currentLanguage: locale }
        ));
      },

      setFallbackLanguage: (locale: LocaleId) => {
        set((state) => (
          state.fallbackLanguage === locale
            ? state
            : { fallbackLanguage: locale }
        ));
      },

      addLoadedNamespace: (namespace: I18nNamespace) => {
        set((state) => (
          state.loadedNamespaces.includes(namespace)
            ? state
            : { loadedNamespaces: [...state.loadedNamespaces, namespace] }
        ));
      },

      setInitialized: (initialized: boolean) => {
        set((state) => (
          state.isInitialized === initialized
            ? state
            : { isInitialized: initialized }
        ));
      },

      setChanging: (changing: boolean) => {
        set((state) => (
          state.isChanging === changing
            ? state
            : { isChanging: changing }
        ));
      },

      setAutoDetect: (autoDetect: boolean) => {
        set((state) => (
          state.autoDetect === autoDetect
            ? state
            : { autoDetect }
        ));
      },

      reset: () => {
        set(initialState);
      },
    }),
    {
      name: 'bitfun-i18n-state',
      partialize: (state) => ({
        currentLanguage: state.currentLanguage,
        fallbackLanguage: state.fallbackLanguage,
        autoDetect: state.autoDetect,
      }),
    }
  )
);


export const selectCurrentLanguage = (state: I18nState) => state.currentLanguage;
export const selectFallbackLanguage = (state: I18nState) => state.fallbackLanguage;
export const selectLoadedNamespaces = (state: I18nState) => state.loadedNamespaces;
export const selectIsInitialized = (state: I18nState) => state.isInitialized;
export const selectIsChanging = (state: I18nState) => state.isChanging;
export const selectAutoDetect = (state: I18nState) => state.autoDetect;
