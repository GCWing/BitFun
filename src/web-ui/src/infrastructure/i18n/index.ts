/**
 * I18n unified exports.
 */

// Core service
export { I18nService, i18nService } from './core/I18nService';

// Provider
export { I18nProvider } from './providers';

// Store
export { useI18nStore } from './store/i18nStore';
export {
  selectCurrentLanguage,
  selectFallbackLanguage,
  selectLoadedNamespaces,
  selectIsInitialized,
  selectIsChanging,
  selectAutoDetect,
} from './store/i18nStore';

// Hooks
export { useI18n, useLanguageSelector, useLanguageDetect } from './hooks';
export type { UseI18nReturn } from './hooks';

// Components
export { LanguageSelector } from './components';
export type { LanguageSelectorProps } from './components';

// Presets
export {
  DEFAULT_LOCALE,
  DEFAULT_FALLBACK_LOCALE,
  DEFAULT_NAMESPACE,
  ALL_NAMESPACES,
  builtinLocales,
  getLocaleFallbackChain,
  getLocaleMetadata,
  isLocaleSupported,
  getSupportedLocaleIds,
  resolveLocaleId,
} from './presets';

// Types
export type {
  LocaleId,
  LocaleMetadata,
  I18nNamespace,
  I18nConfig,
  I18nEventType,
  I18nEvent,
  I18nEventListener,
  I18nHooks,
  I18nState,
  I18nActions,
  TranslationParams,
  PluralOptions,
  DateFormatOptions,
  NumberFormatOptions,
} from './types';
