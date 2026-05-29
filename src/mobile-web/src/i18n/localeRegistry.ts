/**
 * Public mobile-web locale registry.
 *
 * Mobile keeps its own message bundle, but locale identity and aliases come
 * from the shared i18n contract so language detection matches other surfaces.
 */
export {
  DEFAULT_LANGUAGE,
  MOBILE_LOCALES,
  getMobileLanguageShortName,
  getNextMobileLanguage,
  isMobileLanguage,
  resolveMobileLanguage,
} from './generatedLocaleContract';
export type { MobileLanguage } from './generatedLocaleContract';
