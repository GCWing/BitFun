import { describe, expect, it } from 'vitest';
import { isTheme, resolveTheme } from './theme';

describe('Skin Market theme preference', () => {
  it('uses an explicit saved preference', () => {
    expect(resolveTheme('light', true)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
  });

  it('follows the system for missing or invalid preferences', () => {
    expect(resolveTheme(null, false)).toBe('light');
    expect(resolveTheme(null, true)).toBe('dark');
    expect(resolveTheme('system', true)).toBe('dark');
    expect(isTheme('system')).toBe(false);
  });
});
