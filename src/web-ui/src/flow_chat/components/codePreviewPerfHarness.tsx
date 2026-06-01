import { createRoot } from 'react-dom/client';

import { CodePreview } from './CodePreview';

export interface CodePreviewStreamingProbeOptions {
  totalLines?: number;
  updateCount?: number;
  maxHeight?: number;
  autoScrollToBottom?: boolean;
  charsPerLine?: number;
}

export interface CodePreviewStreamingProbeResult {
  renderedLineCount: number;
  startingLineNumber: number;
  containsLine090: boolean;
  containsFinalLine: boolean;
  scrollWrites: number;
  durationMs: number;
  maxFrameMs: number;
  longFrameCount: number;
}

function makeLine(lineNumber: number, charsPerLine: number): string {
  const prefix = `line ${String(lineNumber).padStart(3, '0')} `;
  return prefix + 'x'.repeat(Math.max(0, charsPerLine - prefix.length));
}

function makeContent(lineCount: number, charsPerLine: number): string {
  return Array.from({ length: lineCount }, (_, index) => makeLine(index + 1, charsPerLine)).join('\n');
}

function nextFrame(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => resolve());
  });
}

function getScrollTopDescriptor(): PropertyDescriptor | undefined {
  let prototype: object | null = HTMLElement.prototype;
  while (prototype) {
    const descriptor = Object.getOwnPropertyDescriptor(prototype, 'scrollTop');
    if (descriptor) {
      return descriptor;
    }
    prototype = Object.getPrototypeOf(prototype);
  }
  return undefined;
}

export async function runCodePreviewStreamingPerfProbe({
  totalLines = 160,
  updateCount = 30,
  maxHeight = 88,
  autoScrollToBottom = false,
  charsPerLine = 80,
}: CodePreviewStreamingProbeOptions = {}): Promise<CodePreviewStreamingProbeResult> {
  document.getElementById('bitfun-code-preview-perf-probe')?.remove();

  const host = document.createElement('div');
  host.id = 'bitfun-code-preview-perf-probe';
  host.style.cssText = [
    'position: fixed',
    'left: 0',
    'top: 0',
    'width: 820px',
    'height: 180px',
    'z-index: -1',
    'opacity: 0',
    'pointer-events: none',
  ].join(';');
  document.body.appendChild(host);

  const originalDescriptor = getScrollTopDescriptor();
  let scrollWrites = 0;

  Object.defineProperty(HTMLElement.prototype, 'scrollTop', {
    configurable: true,
    get() {
      return originalDescriptor?.get ? originalDescriptor.get.call(this) : 0;
    },
    set(value: number) {
      if (this instanceof HTMLElement && this.classList.contains('code-preview__content')) {
        scrollWrites += 1;
      }
      originalDescriptor?.set?.call(this, value);
    },
  });

  const root = createRoot(host);
  const frameDurations: number[] = [];
  let lastFrameAt = performance.now();
  const startAt = lastFrameAt;

  try {
    for (let updateIndex = 1; updateIndex <= updateCount; updateIndex += 1) {
      const currentLineCount = Math.max(1, Math.floor((totalLines * updateIndex) / updateCount));
      root.render(
        <CodePreview
          content={makeContent(currentLineCount, charsPerLine)}
          filePath="src/generated.ts"
          isStreaming={true}
          maxHeight={maxHeight}
          autoScrollToBottom={autoScrollToBottom}
        />,
      );

      await nextFrame();
      const now = performance.now();
      frameDurations.push(now - lastFrameAt);
      lastFrameAt = now;
    }

    await nextFrame();
    await nextFrame();

    const text = host.textContent ?? '';
    const renderedLines = text.match(/line \d{3}/g) ?? [];
    const finalLine = `line ${String(totalLines).padStart(3, '0')}`;
    const firstRenderedLine = renderedLines[0]?.match(/\d{3}/)?.[0];

    return {
      renderedLineCount: renderedLines.length,
      startingLineNumber: firstRenderedLine ? Number(firstRenderedLine) : 0,
      containsLine090: text.includes('line 090'),
      containsFinalLine: text.includes(finalLine),
      scrollWrites,
      durationMs: performance.now() - startAt,
      maxFrameMs: Math.max(0, ...frameDurations),
      longFrameCount: frameDurations.filter((duration) => duration > 50).length,
    };
  } finally {
    root.unmount();
    host.remove();
    if (originalDescriptor) {
      Object.defineProperty(HTMLElement.prototype, 'scrollTop', originalDescriptor);
    } else {
      delete (HTMLElement.prototype as { scrollTop?: number }).scrollTop;
    }
  }
}
