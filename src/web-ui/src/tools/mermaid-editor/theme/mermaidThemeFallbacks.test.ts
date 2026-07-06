import { describe, expect, it } from 'vitest';

import {
  MERMAID_THEME_FALLBACKS,
  MERMAID_THEME_MODES,
  getMermaidThemeFallback,
  getMermaidThemeFallbackPair,
} from './mermaidThemeFallbacks';
import { getMermaidConfig } from './mermaidTheme';

describe('mermaid theme fallback palette', () => {
  it('keeps shared node and text fallback roles stable across dark and light themes', () => {
    expect(getMermaidThemeFallback('dark', 'nodeFill')).toBe('#1c1e23');
    expect(getMermaidThemeFallback('light', 'nodeFill')).toBe('#e8eaef');
    expect(getMermaidThemeFallback('dark', 'nodeText')).toBe('#e0e2e8');
    expect(getMermaidThemeFallback('light', 'nodeText')).toBe('#1e293b');
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

  it('reuses compact status and pie fallback colors across theme modes', () => {
    expect(getMermaidThemeFallbackPair('doneStroke')).toEqual({
      dark: '#34d399',
      light: '#34d399',
    });
    expect(getMermaidThemeFallbackPair('critStroke')).toEqual({
      dark: '#ef4444',
      light: '#ef4444',
    });
    expect(getMermaidThemeFallbackPair('pie6')).toEqual({
      dark: '#ec4899',
      light: '#ec4899',
    });
    expect(getMermaidThemeFallbackPair('pie7')).toEqual({
      dark: '#06b6d4',
      light: '#06b6d4',
    });
    expect(getMermaidThemeFallbackPair('pie8')).toEqual({
      dark: '#84cc16',
      light: '#84cc16',
    });
  });

  it('keeps dark info and activation fallbacks visually distinct from neutral notes', () => {
    expect(getMermaidThemeFallback('dark', 'activeStroke')).toBe('#60a5fa');
    expect(getMermaidThemeFallback('dark', 'info')).toBe('#60a5fa');
    expect(getMermaidThemeFallback('dark', 'taskClickableInfo')).toBe('#60a5fa');
    expect(getMermaidThemeFallback('dark', 'activationFill')).toBe(
      'rgba(96, 165, 250, 0.15)'
    );
    expect(getMermaidThemeFallback('dark', 'activationFill')).not.toBe(
      getMermaidThemeFallback('dark', 'noteFill')
    );
  });

  it('feeds compact but distinct fallbacks into Mermaid config without CSS variables', () => {
    const config = getMermaidConfig();
    const themeVariables = config.themeVariables;

    expect(themeVariables.doneTaskBorderColor).toBe('#34d399');
    expect(themeVariables.critBorderColor).toBe('#ef4444');
    expect(themeVariables.activeTaskBorderColor).toBe('#60a5fa');
    expect(themeVariables.taskTextClickableColor).toBe('#60a5fa');
    expect(themeVariables.pie1).toBe('#60a5fa');
    expect(themeVariables.pie6).toBe('#ec4899');
    expect(themeVariables.pie7).toBe('#06b6d4');
    expect(themeVariables.pie8).toBe('#84cc16');
    expect(themeVariables.activationBkgColor).toBe('rgba(96, 165, 250, 0.15)');
    expect(themeVariables.activationBkgColor).not.toBe(themeVariables.noteBkgColor);
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
