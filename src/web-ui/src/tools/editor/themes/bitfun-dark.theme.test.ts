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
    expect(colors['diffEditor.unchangedRegionBackground']).toBe('#121214');
    expect(colors['diffEditor.unchangedCodeBackground']).toBe('#121214');
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

  it('keeps diff text, line, and gutter strengths visually ordered', () => {
    expect(colors['diffEditor.insertedTextBackground']).toBe('#23863625');
    expect(colors['diffEditor.insertedLineBackground']).toBe('#23863630');
    expect(colors['diffEditorGutter.insertedLineBackground']).toBe('#23863638');
    expect(colors['diffEditor.removedTextBackground']).toBe('#DA363325');
    expect(colors['diffEditor.removedLineBackground']).toBe('#DA363330');
    expect(colors['diffEditorGutter.removedLineBackground']).toBe('#DA363338');
    expect(colors['diffEditor.modifiedTextBackground']).toBe('#1F6FEB20');
    expect(colors['diffEditor.modifiedLineBackground']).toBe('#1F6FEB28');
  });
});
