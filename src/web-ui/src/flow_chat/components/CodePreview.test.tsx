import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { CodePreview } from './CodePreview';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/infrastructure/theme', () => ({
  useTheme: () => ({ isLight: false }),
}));

vi.mock('@/infrastructure/language-detection', () => ({
  getPrismLanguage: () => 'typescript',
}));

vi.mock('@/shared/utils/syntaxHighlighterLoader', () => ({
  getLoadedPrismSyntaxHighlighter: () => null,
  loadPrismSyntaxHighlighter: async () => MockSyntaxHighlighter,
}));

function MockSyntaxHighlighter({
  children,
  startingLineNumber = 1,
}: {
  children: string;
  startingLineNumber?: number;
}) {
  return (
    <pre data-testid="syntax-highlighter" data-starting-line-number={startingLineNumber}>
      {children}
    </pre>
  );
}

function makeLines(count: number): string {
  return Array.from({ length: count }, (_, index) =>
    `line ${String(index + 1).padStart(3, '0')}`
  ).join('\n');
}

describe('CodePreview', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
      pretendToBeVisual: true,
    });
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    vi.stubGlobal('cancelAnimationFrame', vi.fn());

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
  });

  it('renders only a viewport-sized tail while streaming long code', async () => {
    await act(async () => {
      root.render(
        <CodePreview
          content={makeLines(120)}
          filePath="src/generated.ts"
          isStreaming={true}
          maxHeight={88}
        />
      );
    });

    const highlighter = container.querySelector('[data-testid="syntax-highlighter"]') as HTMLElement;
    expect(highlighter).not.toBeNull();
    expect(highlighter.textContent).toContain('line 120');
    expect(highlighter.textContent).not.toContain('line 090');
    expect(Number(highlighter.dataset.startingLineNumber)).toBeGreaterThan(100);
  });

  it('keeps enough streaming tail content to fill a taller preview', async () => {
    await act(async () => {
      root.render(
        <CodePreview
          content={makeLines(120)}
          filePath="src/generated.ts"
          isStreaming={true}
          maxHeight={330}
        />
      );
    });

    const highlighter = container.querySelector('[data-testid="syntax-highlighter"]') as HTMLElement;
    expect(highlighter).not.toBeNull();
    expect(highlighter.textContent).toContain('line 120');
    expect(highlighter.textContent).toContain('line 104');
    expect(highlighter.textContent).not.toContain('line 090');
    expect(Number(highlighter.dataset.startingLineNumber)).toBeLessThanOrEqual(104);
  });

  it('fits the streaming tail in the viewport when nested autoscroll is disabled', async () => {
    await act(async () => {
      root.render(
        <CodePreview
          content={makeLines(120)}
          filePath="src/generated.ts"
          isStreaming={true}
          maxHeight={88}
          autoScrollToBottom={false}
        />
      );
    });

    const highlighter = container.querySelector('[data-testid="syntax-highlighter"]') as HTMLElement;
    expect(highlighter).not.toBeNull();
    expect(highlighter.textContent).toContain('line 120');
    expect(highlighter.textContent).toContain('line 117');
    expect(highlighter.textContent).not.toContain('line 116');
    expect(Number(highlighter.dataset.startingLineNumber)).toBe(117);
  });
});
