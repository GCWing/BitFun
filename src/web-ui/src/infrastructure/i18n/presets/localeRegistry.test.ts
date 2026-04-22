import { describe, expect, it } from 'vitest';

import {
  DEFAULT_FALLBACK_LOCALE,
  DEFAULT_LOCALE,
  getSupportedLocaleIds,
  isLocaleSupported,
} from './index';
import { builtinLocales, LOCALE_IDS } from './localeRegistry';

describe('localeRegistry', () => {
  it('keeps locale ids and metadata in the same order', () => {
    expect(builtinLocales.map(locale => locale.id)).toEqual([...LOCALE_IDS]);
  });

  it('contains unique ids with complete display metadata', () => {
    const ids = builtinLocales.map(locale => locale.id);
    expect(new Set(ids).size).toBe(ids.length);

    for (const locale of builtinLocales) {
      expect(locale.name.length).toBeGreaterThan(0);
      expect(locale.englishName.length).toBeGreaterThan(0);
      expect(locale.nativeName.length).toBeGreaterThan(0);
      expect(locale.dateFormat.length).toBeGreaterThan(0);
      expect(locale.numberFormat.decimal.length).toBeGreaterThan(0);
      expect(locale.numberFormat.thousands.length).toBeGreaterThan(0);
    }
  });

  it('derives support checks from the registry', () => {
    expect(getSupportedLocaleIds()).toEqual([...LOCALE_IDS]);
    expect(isLocaleSupported(DEFAULT_LOCALE)).toBe(true);
    expect(isLocaleSupported(DEFAULT_FALLBACK_LOCALE)).toBe(true);
    expect(isLocaleSupported('fr-FR')).toBe(false);
  });
});
