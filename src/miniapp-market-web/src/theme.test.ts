import { describe, expect, it } from 'vitest';
import { isTheme, resolveTheme } from './theme';

describe('market theme preference', () => {
  it('uses a saved light or dark preference', () => {
    expect(resolveTheme('light', true)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
  });

  it('follows the system when no valid preference is saved', () => {
    expect(resolveTheme(null, false)).toBe('light');
    expect(resolveTheme(null, true)).toBe('dark');
    expect(resolveTheme('system', true)).toBe('dark');
  });

  it('only accepts explicit light and dark values', () => {
    expect(isTheme('light')).toBe(true);
    expect(isTheme('dark')).toBe(true);
    expect(isTheme('system')).toBe(false);
    expect(isTheme(null)).toBe(false);
  });
});
