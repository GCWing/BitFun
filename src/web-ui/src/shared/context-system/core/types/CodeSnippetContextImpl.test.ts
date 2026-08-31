import { describe, expect, it } from 'vitest';

import {
  APPEARANCE_DOMAIN_TOKENS,
  APPEARANCE_LANGUAGE_TOKENS,
} from '@/infrastructure/appearance/appearanceDomainTokens';
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
    expect(getLanguageColor('typescript')).toBe(APPEARANCE_LANGUAGE_TOKENS.typescript);
    expect(getLanguageColor('rust')).toBe(APPEARANCE_LANGUAGE_TOKENS.rust);
    expect(getLanguageColor('java')).toBe(APPEARANCE_LANGUAGE_TOKENS.java);
    expect(getLanguageColor('unknown-lang')).toBe(APPEARANCE_LANGUAGE_TOKENS.plaintext);
    expect(getLanguageColor()).toBe(APPEARANCE_LANGUAGE_TOKENS.plaintext);
  });

  it('keeps mermaid diagram context color in the UI exception registry', () => {
    expect(APPEARANCE_DOMAIN_TOKENS.mermaidDiagram).toBe(
      'var(--bf-domain-mermaid-diagram)',
    );
  });
});
