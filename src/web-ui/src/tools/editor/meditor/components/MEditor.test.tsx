import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { MEditor } from './MEditor';

vi.mock('@/shared/utils/logger', () => ({
  createLogger: vi.fn(),
}));

vi.mock('@/tools/editor/services/ActiveEditTargetService', () => ({
  activeEditTargetService: {
    bindTarget: vi.fn(),
    clearActiveTarget: vi.fn(),
    setActiveTarget: vi.fn(),
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock('../utils/tiptapMarkdown', () => ({
  analyzeMarkdownEditability: () => ({ containsRenderOnlyBlocks: false }),
}));

vi.mock('./Preview', () => ({
  Preview: ({ value }: { value: string }) => <div data-testid="preview">{value.length}</div>,
}));

vi.mock('./TiptapEditor', () => ({
  TiptapEditor: () => <div data-testid="tiptap" />,
}));

vi.mock('./EditArea', () => ({
  EditArea: () => <textarea data-testid="edit-area" />,
}));

describe('MEditor initial mode', () => {
  it('renders a large preview directly without mounting Tiptap on the first frame', () => {
    const markdown = 'x'.repeat(2 * 1024 * 1024);
    const html = renderToStaticMarkup(
      <MEditor value={markdown} mode="preview" progressivePreview readonly />,
    );

    expect(html).toContain('data-testid="preview"');
    expect(html).not.toContain('data-testid="tiptap"');
  });
});