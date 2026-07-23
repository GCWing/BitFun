import { describe, expect, it } from 'vitest';
import { isWindowFullscreenShortcut } from './windowFullscreenShortcut';

const event = (
  key: string,
  overrides: Partial<Pick<KeyboardEvent, 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>> = {}
) => ({
  key,
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...overrides,
} as KeyboardEvent);

describe('isWindowFullscreenShortcut', () => {
  it('uses F11 for Windows and Linux OS-window fullscreen', () => {
    expect(isWindowFullscreenShortcut(event('F11'), 'Win32')).toBe(true);
    expect(isWindowFullscreenShortcut(event('F11'), 'Linux x86_64')).toBe(true);
  });

  it('does not treat modified F11 as the Desktop fullscreen shortcut', () => {
    expect(isWindowFullscreenShortcut(event('F11', { ctrlKey: true }), 'Win32')).toBe(false);
    expect(isWindowFullscreenShortcut(event('F11', { shiftKey: true }), 'Linux x86_64')).toBe(false);
  });

  it('uses Control+Command+F on macOS', () => {
    expect(isWindowFullscreenShortcut(event('f', { ctrlKey: true, metaKey: true }), 'MacIntel')).toBe(true);
    expect(isWindowFullscreenShortcut(event('F', { ctrlKey: true, metaKey: true }), 'MacIntel')).toBe(true);
  });

  it('does not confuse macOS Command+F with OS-window fullscreen', () => {
    expect(isWindowFullscreenShortcut(event('f', { metaKey: true }), 'MacIntel')).toBe(false);
    expect(isWindowFullscreenShortcut(event('f', { ctrlKey: true }), 'MacIntel')).toBe(false);
  });
});
