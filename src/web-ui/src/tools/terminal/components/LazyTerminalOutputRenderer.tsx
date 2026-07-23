import React, { forwardRef, Suspense } from 'react';
import type {
  TerminalOutputRendererHandle,
  TerminalOutputRendererProps,
} from './TerminalOutputRenderer';

const DeferredTerminalOutputRenderer = React.lazy(() =>
  import('./TerminalOutputRenderer').then((module) => ({
    default: module.TerminalOutputRenderer,
  }))
);
const FALLBACK_OUTPUT_FONT_SIZE = 12;
const FALLBACK_OUTPUT_LINE_HEIGHT = 1.4;
const FALLBACK_OUTPUT_ROW_HEIGHT = Math.ceil(FALLBACK_OUTPUT_FONT_SIZE * FALLBACK_OUTPUT_LINE_HEIGHT);

function stripTerminalControlSequences(content: string): string {
  return content
    // eslint-disable-next-line no-control-regex -- terminal control sequences are expected in command output.
    .replace(/\x1b[\]PX_^][\s\S]*?(?:\x07|\x1b\\)/g, '')
    // eslint-disable-next-line no-control-regex -- terminal control sequences are expected in command output.
    .replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, '')
    // eslint-disable-next-line no-control-regex -- terminal control sequences are expected in command output.
    .replace(/\x1b[ -/]*[@-~]/g, '');
}

function takeLastRows(content: string, maxRows?: number): string {
  if (!maxRows || maxRows <= 0) {
    return content;
  }

  let rowCount = 0;
  let cursor = content.length;
  while (cursor > 0 && rowCount < maxRows) {
    const previousBreak = content.lastIndexOf('\n', cursor - 1);
    if (previousBreak < 0) {
      return content;
    }
    rowCount += 1;
    cursor = previousBreak;
  }

  return content.slice(cursor + 1);
}

function calculateFallbackHeight(content: string, maxRows?: number, minHeight = FALLBACK_OUTPUT_ROW_HEIGHT, maxHeight = 300): number {
  const lines = content ? content.split(/\r\n|\r|\n/) : [''];
  const visibleRows = maxRows != null && maxRows > 0
    ? Math.min(lines.length, maxRows)
    : lines.length;
  const estimatedHeight = Math.max(1, visibleRows) * FALLBACK_OUTPUT_ROW_HEIGHT;
  const effectiveMaxHeight = maxRows != null && maxRows > 0
    ? maxRows * FALLBACK_OUTPUT_ROW_HEIGHT
    : maxHeight;

  return Math.min(Math.max(estimatedHeight, minHeight), Math.max(minHeight, effectiveMaxHeight));
}

export function TerminalOutputFallback({
  className,
  content,
  minHeight,
  maxHeight,
  maxRows,
}: Pick<TerminalOutputRendererProps, 'className' | 'content' | 'minHeight' | 'maxHeight' | 'maxRows'>) {
  const preview = stripTerminalControlSequences(takeLastRows(content, maxRows));
  const height = calculateFallbackHeight(preview, maxRows, minHeight, maxHeight);

  return (
    <pre
      className={['terminal-output-pre', className].filter(Boolean).join(' ')}
      style={{
        height: `${height}px`,
        maxHeight: `${height}px`,
        overflow: 'hidden',
      }}
    >
      {preview}
    </pre>
  );
}

export const LazyTerminalOutputRenderer = forwardRef<
  TerminalOutputRendererHandle,
  TerminalOutputRendererProps
>((props, ref) => (
  <Suspense fallback={<TerminalOutputFallback {...props} />}>
    <DeferredTerminalOutputRenderer {...props} ref={ref} />
  </Suspense>
));

LazyTerminalOutputRenderer.displayName = 'LazyTerminalOutputRenderer';

export type {
  TerminalOutputRendererHandle,
  TerminalOutputRendererProps,
};
