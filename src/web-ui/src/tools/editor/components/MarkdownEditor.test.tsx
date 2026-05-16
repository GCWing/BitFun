import React, { forwardRef, useImperativeHandle } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import MarkdownEditor from './MarkdownEditor';

function Icon({ name }: { name: string }) {
  return <svg data-icon={name} />;
}

vi.mock('lucide-react', () => ({
  AlertCircle: () => <Icon name="alert-circle" />,
  Check: () => <Icon name="check" />,
  Copy: () => <Icon name="copy" />,
}));

vi.mock('@/component-library', () => ({
  Button: ({
    children,
    className,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button type="button" className={className} {...props}>
      {children}
    </button>
  ),
  CubeLoading: ({ text }: { text: string }) => <div>{text}</div>,
}));

vi.mock('../meditor', () => ({
  MEditor: forwardRef((_props: { value?: string }, ref) => {
    useImperativeHandle(ref, () => ({
      destroy: vi.fn(),
      markSaved: vi.fn(),
      setInitialContent: vi.fn(),
    }));
    return <div data-testid="markdown-body" />;
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

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
  }),
}));

vi.mock('@/infrastructure/theme/hooks/useTheme', () => ({
  useTheme: () => ({ isLight: false }),
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

vi.mock('@/component-library/components/ConfirmDialog/confirmService', () => ({
  confirmDialog: vi.fn(),
}));

describe('MarkdownEditor', () => {
  it('renders a compact copy action in the toolbar', () => {
    const html = renderToStaticMarkup(
      <MarkdownEditor initialContent="# Deep Review\n\nReady." />,
    );

    expect(html).toContain('aria-label="Copy Markdown"');
    expect(html).toContain('data-icon="copy"');
    expect(html).toContain('bitfun-markdown-editor__toolbar-button');
  });
});
