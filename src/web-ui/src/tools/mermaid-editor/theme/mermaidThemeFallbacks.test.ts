import { describe, expect, it } from 'vitest';

import {
  MERMAID_THEME_FALLBACKS,
  MERMAID_THEME_MODES,
  getMermaidThemeFallback,
  getMermaidThemeFallbackPair,
} from './mermaidThemeFallbacks';

describe('mermaid theme fallback palette', () => {
  it('keeps shared node and text fallback roles stable across dark and light themes', () => {
    expect(getMermaidThemeFallback('dark', 'nodeFill')).toBe('#1c1e23');
    expect(getMermaidThemeFallback('light', 'nodeFill')).toBe('#e8eaef');
    expect(getMermaidThemeFallback('dark', 'nodeText')).toBe('#e0e2e8');
    expect(getMermaidThemeFallback('light', 'nodeText')).toBe('#1f2937');
  });

  it('returns paired fallbacks for CSS variable resolution without duplicating call-site literals', () => {
    expect(getMermaidThemeFallbackPair('edgeStroke')).toEqual({
      dark: '#5a5e6a',
      light: '#9ca3af',
    });
  });

  it('keeps mixed Mermaid fallback roles separate when dark and light values diverge', () => {
    expect(getMermaidThemeFallbackPair('nodeBorder')).toEqual({
      dark: '#4a4e58',
      light: '#9ca3af',
    });
    expect(getMermaidThemeFallbackPair('sectionFill')).toEqual({
      dark: '#1c1e23',
      light: '#f3f4f6',
    });
  });

  it('merges indistinguishable Mermaid fallback colors without removing semantic roles', () => {
    expect(getMermaidThemeFallback('light', 'nodeFillHover')).toBe('#e0e2e8');
    expect(getMermaidThemeFallback('dark', 'edgeLabelBorderHover')).toBe(
      getMermaidThemeFallback('dark', 'nodeStroke')
    );
    expect(getMermaidThemeFallback('dark', 'textMuted')).toBe(
      getMermaidThemeFallback('dark', 'nodeStrokeHover')
    );
  });

  it('keeps every theme mode on the same fallback key contract', () => {
    const fallbackKeys = Object.keys(MERMAID_THEME_FALLBACKS.dark).sort();

    expect(Object.keys(MERMAID_THEME_FALLBACKS.light).sort()).toEqual(fallbackKeys);

    for (const mode of MERMAID_THEME_MODES) {
      for (const [key, value] of Object.entries(MERMAID_THEME_FALLBACKS[mode])) {
        expect(value, `${mode}.${key}`).toMatch(/\S/);
      }
    }
  });
});
