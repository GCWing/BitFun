/**
 * Single Web UI registry for selectable locales.
 *
 * To add a locale, add one metadata entry here and provide the matching
 * `src/web-ui/src/locales/<locale-id>/*.json` files. The i18n audit checks
 * that the registry and locale folders stay in sync.
 */
export const LOCALE_IDS = ['zh-CN', 'zh-TW', 'en-US'] as const;
export type LocaleId = (typeof LOCALE_IDS)[number];

export const builtinLocales = [
  {
    id: 'zh-CN',
    name: '简体中文',
    englishName: 'Simplified Chinese',
    nativeName: '简体中文',
    rtl: false,
    dateFormat: 'YYYY年MM月DD日',
    numberFormat: {
      decimal: '.',
      thousands: ',',
    },
    builtin: true,
  },
  {
    id: 'zh-TW',
    name: '繁體中文',
    englishName: 'Traditional Chinese',
    nativeName: '繁體中文',
    rtl: false,
    dateFormat: 'YYYY年MM月DD日',
    numberFormat: {
      decimal: '.',
      thousands: ',',
    },
    builtin: true,
  },
  {
    id: 'en-US',
    name: 'English',
    englishName: 'English (US)',
    nativeName: 'English',
    rtl: false,
    dateFormat: 'MM/DD/YYYY',
    numberFormat: {
      decimal: '.',
      thousands: ',',
    },
    builtin: true,
  },
] satisfies LocaleMetadata[];

export interface LocaleMetadata {
  id: LocaleId;
  name: string;
  englishName: string;
  nativeName: string;
  rtl: boolean;
  dateFormat: string;
  numberFormat: {
    decimal: string;
    thousands: string;
  };
  builtin: boolean;
}
