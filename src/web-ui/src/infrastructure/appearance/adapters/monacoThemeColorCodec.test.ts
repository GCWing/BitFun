import { describe, expect, it } from 'vitest';
import type { MonacoAppearanceSettings } from '../types';
import {
  isMonacoAppearanceColor,
  projectMonacoAppearanceSettings,
} from './monacoThemeColorCodec';

function createSettings(
  overrides: Partial<MonacoAppearanceSettings> = {},
): MonacoAppearanceSettings {
  return {
    id: 'test-theme',
    base: 'vs',
    inherit: true,
    rules: [],
    colors: {},
    ...overrides,
  };
}

describe('Monaco theme color codec', () => {
  it('projects CSS alpha colors into Monaco-native color formats', () => {
    const settings = createSettings({
      rules: [
        { token: 'comment', foreground: 'rgba(255, 0, 0, 0.5)' },
        { token: 'keyword', foreground: '6b5a89' },
      ],
      colors: {
        'editor.background': '#ffffff',
        'editor.foreground': 'rgba(0, 0, 0, 0.80)',
        'editor.selectionBackground': 'rgba(16, 26, 39, 0.14)',
        'editorCursor.foreground': '#abc',
      },
    });

    const projected = projectMonacoAppearanceSettings(settings);

    expect(projected.colors).toEqual({
      'editor.background': '#ffffff',
      'editor.foreground': '#333333',
      'editor.selectionBackground': '#101a2724',
      'editorCursor.foreground': '#aabbcc',
    });
    expect(projected.rules).toEqual([
      { token: 'comment', foreground: '#ff8080' },
      { token: 'keyword', foreground: '#6b5a89' },
    ]);
    expect(settings.colors['editor.foreground']).toBe('rgba(0, 0, 0, 0.80)');
  });

  it('requires an explicit opaque canvas for colors that need alpha compositing', () => {
    expect(() => projectMonacoAppearanceSettings(createSettings({
      colors: { 'editor.background': 'rgba(0, 0, 0, 0.5)' },
    }))).toThrow('colors.editor.background must be opaque');
    expect(() => projectMonacoAppearanceSettings(createSettings({
      colors: { 'editor.foreground': 'rgba(255, 255, 255, 0.5)' },
    }))).toThrow('colors.editor.foreground uses alpha');
  });

  it('rejects out-of-range channels while preserving supported legacy inputs', () => {
    expect(isMonacoAppearanceColor('rgba(0, 0, 0, 0.80)')).toBe(true);
    expect(isMonacoAppearanceColor('6b5a89', true)).toBe(true);
    expect(isMonacoAppearanceColor('6b5a89')).toBe(false);
    expect(isMonacoAppearanceColor('rgba(256, 0, 0, 0.5)')).toBe(false);
    expect(isMonacoAppearanceColor('rgba(0, 0, 0, 1.1)')).toBe(false);
  });
});
