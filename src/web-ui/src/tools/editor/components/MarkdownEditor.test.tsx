import React, { forwardRef, useImperativeHandle } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import MarkdownEditor from './MarkdownEditor';

vi.mock('../meditor', () => ({
  MEditor: forwardRef((props: { value?: string; mode?: string }, ref) => {
    useImperativeHandle(ref, () => ({
      destroy: vi.fn(),
      markSaved: vi.fn(),
      setInitialContent: vi.fn(),
    }));
    return <div data-testid="markdown-body" data-mode={props.mode} />;
  }),
}));

vi.mock('./CodeEditor', () => ({
  default: () => <div data-testid="code-editor" />,
}));

vi.mock('../meditor/utils/tiptapMarkdown', () => ({
  analyzeMarkdownEditability: (raw: string) => ({
    canonicalMarkdown: raw,
    containsRawHtmlInlines: false,
    containsRenderOnlyBlocks: false,
    mode: 'safe',
  }),
}));

const messages: Record<string, string> = {
  'editor.markdownEditor.copiedMarkdown': 'Copied Markdown',
  'editor.markdownEditor.copyMarkdown': 'Copy Markdown',
  'editor.markdownEditor.notice.sourcePreviewFallback': 'IR fallback warning',
};

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string, options?: { defaultValue?: string }) => messages[key] ?? options?.defaultValue ?? key,
  }),
}));

vi.mock('@/infrastructure/appearance', () => ({
  useAppearance: () => ({ current: { mode: 'dark' } }),
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
  }),
}));

vi.mock('@/shared/utils/debugProbe', () => ({
  sendDebugProbe: vi.fn(),
}));

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: {
    emit: vi.fn(),
    on: vi.fn(() => vi.fn()),
  },
}));

vi.mock('@/infrastructure/confirm-dialog', () => ({
  confirmDialog: vi.fn(),
}));

describe('MarkdownEditor', () => {
  it('renders a compact copy action in the toolbar', () => {
    const html = renderToStaticMarkup(
      <MarkdownEditor initialContent="# Deep Review\n\nReady." />,
    );

    expect(html).toContain('aria-label="Copy Markdown"');
    expect(html).toContain('data-openbitfun-component="toolbar"');
    expect(html).toContain('data-openbitfun-component="icon-button"');
  });

  it('opens Mermaid documents in rich text mode', () => {
    const html = renderToStaticMarkup(
      <MarkdownEditor initialContent="```mermaid\ngraph TD\n  A-->B\n```" />,
    );

    expect(html).toContain('data-mode="ir"');
  });

  it('offers only rich text and source modes', () => {
    const html = renderToStaticMarkup(
      <MarkdownEditor initialContent="# Ordinary Markdown" />,
    );

    expect(html).toContain('editor.markdownEditor.richText');
    expect(html).toContain('editor.markdownEditor.source');
    expect(html).not.toContain('editor.markdownEditor.preview');
    expect(html.match(/role="radio"/g)).toHaveLength(2);
  });
});
