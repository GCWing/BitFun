import { describe, expect, it } from 'vitest';
import { splitGlobalSearchShortcutLabel } from './globalSearchShortcut';

describe('splitGlobalSearchShortcutLabel', () => {
  it.each([
    ['Ctrl+K', { modifier: 'Ctrl', key: 'K' }],
    ['Ctrl+Shift+K', { modifier: 'Ctrl Shift', key: 'K' }],
    ['⌘K', { modifier: '⌘', key: 'K' }],
    ['⌘⇧K', { modifier: '⌘⇧', key: 'K' }],
    ['K', { key: 'K' }],
  ])('splits %s into compact hint parts', (label, expected) => {
    expect(splitGlobalSearchShortcutLabel(label)).toEqual(expected);
  });
});
