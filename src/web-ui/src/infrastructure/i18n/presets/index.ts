 

import type { LocaleId } from './localeRegistry';
import {
  builtinLocales,
  getLocaleFallbackChain,
  getLocaleMetadata,
  getSupportedLocaleIds,
  isLocaleSupported,
  resolveLocaleId,
} from './localeRegistry';
export { ALL_NAMESPACES } from './namespaceRegistry';
export {
  builtinLocales,
  getLocaleFallbackChain,
  getLocaleMetadata,
  getSupportedLocaleIds,
  isLocaleSupported,
  resolveLocaleId,
};
export type { LocaleId };

export const DEFAULT_LOCALE = 'zh-CN' satisfies LocaleId;

export const DEFAULT_FALLBACK_LOCALE = 'en-US' satisfies LocaleId;

export const DEFAULT_NAMESPACE = 'common';
