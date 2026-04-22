 

import { builtinLocales } from './localeRegistry';
import type { LocaleId, LocaleMetadata } from './localeRegistry';
export { ALL_NAMESPACES } from './namespaceRegistry';
export { builtinLocales };

export const DEFAULT_LOCALE = 'zh-CN' satisfies LocaleId;

export const DEFAULT_FALLBACK_LOCALE = 'en-US' satisfies LocaleId;

 
export function getLocaleMetadata(localeId: LocaleId): LocaleMetadata | undefined {
  return builtinLocales.find(locale => locale.id === localeId);
}

 
export function isLocaleSupported(localeId: string): localeId is LocaleId {
  return builtinLocales.some(locale => locale.id === localeId);
}

 
export function getSupportedLocaleIds(): LocaleId[] {
  return builtinLocales.map(locale => locale.id);
}

 
export const DEFAULT_NAMESPACE = 'common';
