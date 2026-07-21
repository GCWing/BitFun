import type { CanvasDiffLine, CanvasDiffViewProps } from './types';

export function normalizeDiffLines(lines: CanvasDiffViewProps['lines']): CanvasDiffLine[] {
  const rawLines = typeof lines === 'string' ? lines.split('\n') : Array.isArray(lines) ? lines : [];
  return rawLines.map((line, index) => {
    if (line && typeof line === 'object' && !Array.isArray(line)) {
      return {
        type: line.type,
        lineNumber: line.lineNumber ?? line.oldLineNumber ?? line.newLineNumber ?? index + 1,
        content: line.content ?? line.text ?? '',
      };
    }
    const content = String(line ?? '');
    const added = content.startsWith('+') && !content.startsWith('+++');
    const removed = content.startsWith('-') && !content.startsWith('---');
    return {
      type: added ? 'added' : removed ? 'removed' : undefined,
      lineNumber: index + 1,
      content: added || removed ? content.slice(1) : content,
    };
  });
}
