import { describe, expect, it } from 'vitest';
import { BitFunDarkTheme } from './bitfun-dark.theme';

describe('BitFunDarkTheme color roles', () => {
  const colors = BitFunDarkTheme.colors;

  it('keeps editor surface roles output-equivalent', () => {
    expect(colors['editor.background']).toBe('#121214');
    expect(colors['editor.lineHighlightBackground']).toBe('#18181a');
    expect(colors['editor.lineHighlightBorder']).toBe('#202024');
    expect(colors['editorWidget.background']).toBe('#18181a');
    expect(colors['editorHoverWidget.statusBarBackground']).toBe('#202024');
    expect(colors['diffEditor.unchangedRegionBackground']).toBe('#0D0D0F');
    expect(colors['diffEditor.unchangedCodeBackground']).toBe('#0D0D0F');
  });

  it('keeps BitFun accent roles output-equivalent', () => {
    expect(colors['editorCursor.foreground']).toBe('#E1AB80');
    expect(colors['editor.selectionBackground']).toBe('#E1AB8040');
    expect(colors['editor.inactiveSelectionBackground']).toBe('#E1AB8020');
    expect(colors['editor.wordHighlightBorder']).toBe('#E1AB8060');
    expect(colors['scrollbarSlider.hoverBackground']).toBe('#E1AB8070');
    expect(colors['scrollbarSlider.activeBackground']).toBe('#E1AB80A0');
  });

  it('keeps repeated editor semantic colors aligned', () => {
    expect(colors['editorInlayHint.foreground']).toBe(colors['editorCodeLens.foreground']);
    expect(colors['editorError.foreground']).toBe(colors['minimap.errorHighlight']);
    expect(colors['editorWarning.foreground']).toBe(colors['minimap.warningHighlight']);
    expect(colors['editorLink.activeForeground']).toBe('#7DCFFF');
  });
});
