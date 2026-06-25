import { describe, expect, it } from 'vitest';

import { UI_EXCEPTION_ACCENTS } from '@/shared/theme/uiExceptionAccents';
import { getLanguageColor, getLanguageDisplayName } from './CodeSnippetContextImpl';

describe('CodeSnippetContextImpl language metadata', () => {
  it('preserves code snippet display name mapping', () => {
    expect(getLanguageDisplayName('typescript')).toBe('TypeScript');
    expect(getLanguageDisplayName('cpp')).toBe('C++');
    expect(getLanguageDisplayName('tsx')).toBe('tsx');
    expect(getLanguageDisplayName('unknown-lang')).toBe('unknown-lang');
    expect(getLanguageDisplayName()).toBe('Text');
  });

  it('keeps code snippet language accents centralized', () => {
    expect(getLanguageColor('typescript')).toBe('#3178c6');
    expect(getLanguageColor('rust')).toBe('var(--color-bg-primary)');
    expect(getLanguageColor('java')).toBe('#007396');
    expect(getLanguageColor('unknown-lang')).toBe('#858585');
    expect(getLanguageColor()).toBe('#858585');
  });

  it('keeps mermaid diagram context color in the UI exception registry', () => {
    expect(UI_EXCEPTION_ACCENTS.mermaidDiagram).toBe('#22c55e');
  });
});
