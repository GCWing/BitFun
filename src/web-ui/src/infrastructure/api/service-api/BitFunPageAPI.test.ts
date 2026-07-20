import { describe, expect, it } from 'vitest';
import { isValidPageSlug } from './BitFunPageAPI';

describe('isValidPageSlug', () => {
  it('accepts lowercase alphanumeric slugs with hyphens', () => {
    expect(isValidPageSlug('a')).toBe(true);
    expect(isValidPageSlug('my-site')).toBe(true);
    expect(isValidPageSlug('site123')).toBe(true);
  });

  it('rejects invalid slugs', () => {
    expect(isValidPageSlug('')).toBe(false);
    expect(isValidPageSlug('-bad')).toBe(false);
    expect(isValidPageSlug('Bad')).toBe(false);
    expect(isValidPageSlug('has_underscore')).toBe(false);
    expect(isValidPageSlug('a'.repeat(65))).toBe(false);
  });
});
