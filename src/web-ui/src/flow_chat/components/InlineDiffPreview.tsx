/**
 * InlineDiffPreview component.
 * Lightweight inline diff preview for tool cards.
 *
 * Design notes:
 * 1. Avoid Monaco DiffEditor (too heavy)
 * 2. Use the diff library (npm: diff) for performance
 * 3. GitHub-style unified diff view
 * 4. Syntax highlighting via react-syntax-highlighter
 * 5. Timeout protection for large files
 */

import React, { useMemo, memo, useRef, useCallback, useState } from 'react';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { diffLines, Change } from 'diff';
import { getPrismLanguage } from '@/infrastructure/language-detection';
import { createLogger } from '@/shared/utils/logger';
import './InlineDiffPreview.scss';

const log = createLogger('InlineDiffPreview');

export interface InlineDiffPreviewProps {
  /** Original content. */
  originalContent: string;
  /** Modified content. */
  modifiedContent: string;
  /** File path (language detection). */
  filePath?: string;
  /** Explicit language. */
  language?: string;
  /** Max height in px. */
  maxHeight?: number;
  /** Custom class name. */
  className?: string;
  /** Whether to show line numbers. */
  showLineNumbers?: boolean;
  /** Line number mode: dual=two columns, single=one column. */
  lineNumberMode?: 'dual' | 'single';
  /** Whether to show +/- prefix. */
  showPrefix?: boolean;
  /** Context lines around changes. */
  contextLines?: number;
  /** Row click callback. */
  onLineClick?: (lineNumber: number, type: 'original' | 'modified') => void;
}

/** Diff line type. */
type DiffLineType = 'unchanged' | 'added' | 'removed' | 'context-separator';

/** Diff line data. */
interface DiffLine {
  type: DiffLineType;
  content: string;
  originalLineNumber?: number;
  modifiedLineNumber?: number;
}

/**
 * Compute line-level diff using the diff library.
 * More performant than a custom LCS implementation (O(ND) vs O(nm)).
 */
function computeLineDiff(originalContent: string, modifiedContent: string): DiffLine[] {
  const result: DiffLine[] = [];
  
  const changes: Change[] = diffLines(originalContent, modifiedContent);
  
  let originalLineNumber = 1;
  let modifiedLineNumber = 1;
  
  for (const change of changes) {
    // Split into lines and drop the trailing empty line from split().
    const lines = change.value.split('\n');
    if (lines.length > 0 && lines[lines.length - 1] === '') {
      lines.pop();
    }
    
    for (const line of lines) {
      if (change.added) {
        result.push({
          type: 'added',
          content: line,
          modifiedLineNumber: modifiedLineNumber++,
        });
      } else if (change.removed) {
        result.push({
          type: 'removed',
          content: line,
          originalLineNumber: originalLineNumber++,
        });
      } else {
        result.push({
          type: 'unchanged',
          content: line,
          originalLineNumber: originalLineNumber++,
          modifiedLineNumber: modifiedLineNumber++,
        });
      }
    }
  }
  
  return result;
}

/**
 * Diff computation with timeout protection.
 */
async function computeDiffWithTimeout(
  originalContent: string,
  modifiedContent: string,
  timeout: number = 5000
): Promise<DiffLine[]> {
  return new Promise((resolve, reject) => {
    const timeoutId = setTimeout(() => {
      reject(new Error('Diff computation timeout'));
    }, timeout);
    
    try {
      const result = computeLineDiff(originalContent, modifiedContent);
      clearTimeout(timeoutId);
      resolve(result);
    } catch (error) {
      clearTimeout(timeoutId);
      reject(error);
    }
  });
}

/**
 * Context-collapsed diff view.
 * Hides long unchanged sections and keeps context around changes.
 */
function applyContextCollapsing(diffLines: DiffLine[], contextLines: number): DiffLine[] {
  if (contextLines < 0) return diffLines; // Negative means show all lines.
  
  const result: DiffLine[] = [];
  const changeIndices: number[] = [];
  
  diffLines.forEach((line, index) => {
    if (line.type === 'added' || line.type === 'removed') {
      changeIndices.push(index);
    }
  });
  
  if (changeIndices.length === 0) {
    return [{
      type: 'context-separator',
      content: 'No differences; contents are identical.',
    }];
  }
  
  const showLine = new Set<number>();
  for (const idx of changeIndices) {
    for (let i = Math.max(0, idx - contextLines); i <= Math.min(diffLines.length - 1, idx + contextLines); i++) {
      showLine.add(i);
    }
  }
  
  let lastShownIndex = -1;
  for (let i = 0; i < diffLines.length; i++) {
    if (showLine.has(i)) {
      if (lastShownIndex >= 0 && i > lastShownIndex + 1) {
        const skippedCount = i - lastShownIndex - 1;
        result.push({
          type: 'context-separator',
          content: `... omitted ${skippedCount} lines ...`,
        });
      }
      result.push(diffLines[i]);
      lastShownIndex = i;
    }
  }
  
  if (result.length > 0 && result[0].type !== 'context-separator') {
    const firstShownIdx = Array.from(showLine).sort((a, b) => a - b)[0];
    if (firstShownIdx > 0) {
      result.unshift({
        type: 'context-separator',
        content: `... omitted first ${firstShownIdx} lines ...`,
      });
    }
  }
  
  if (lastShownIndex < diffLines.length - 1) {
    const skippedCount = diffLines.length - 1 - lastShownIndex;
    result.push({
      type: 'context-separator',
      content: `... omitted last ${skippedCount} lines ...`,
    });
  }
  
  return result;
}

/**
 * Custom theme styles (kept in sync with CodePreview).
 */
const customStyle = {
  ...vscDarkPlus,
  'pre[class*="language-"]': {
    ...vscDarkPlus['pre[class*="language-"]'],
    margin: 0,
    padding: 0,
    background: 'transparent',
    fontSize: '12px',
    lineHeight: '1.6',
    fontFamily: "'JetBrains Mono', 'Fira Code', 'SF Mono', 'Consolas', 'Monaco', 'Courier New', monospace",
  },
  'code[class*="language-"]': {
    ...vscDarkPlus['code[class*="language-"]'],
    background: 'transparent',
    fontSize: '12px',
    lineHeight: '1.6',
    fontFamily: "'JetBrains Mono', 'Fira Code', 'SF Mono', 'Consolas', 'Monaco', 'Courier New', monospace",
  },
};

/**
 * InlineDiffPreview component.
 */
export const InlineDiffPreview: React.FC<InlineDiffPreviewProps> = memo(({
  originalContent,
  modifiedContent,
  filePath,
  language,
  maxHeight = 300,
  className = '',
  showLineNumbers = true,
  lineNumberMode = 'dual',
  showPrefix = true,
  contextLines = 3,
  onLineClick,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [highlightedLine, setHighlightedLine] = useState<number | null>(null);
  
  const detectedLanguage = useMemo(() => {
    if (language) return language;
    if (filePath) return getPrismLanguage(filePath);
    return 'text';
  }, [language, filePath]);
  
  const diffLines = useMemo(() => {
    try {
      const rawDiff = computeLineDiff(originalContent, modifiedContent);
      return applyContextCollapsing(rawDiff, contextLines);
    } catch (error) {
      log.error('Diff computation failed', error);
      return [{
        type: 'context-separator' as const,
        content: 'Diff computation failed; file may be too large.',
      }];
    }
  }, [originalContent, modifiedContent, contextLines]);
  
  const handleLineClick = useCallback((index: number, line: DiffLine) => {
    if (line.type === 'context-separator') return;
    
    setHighlightedLine(prev => prev === index ? null : index);
    
    if (onLineClick) {
      const lineNum = line.type === 'removed' ? line.originalLineNumber : line.modifiedLineNumber;
      const type = line.type === 'removed' ? 'original' : 'modified';
      if (lineNum) {
        onLineClick(lineNum, type);
      }
    }
  }, [onLineClick]);
  
  const renderLine = useCallback((line: DiffLine, index: number) => {
    if (line.type === 'context-separator') {
      return (
        <div key={`sep-${index}`} className="diff-line diff-line--separator">
          <span className="diff-line__gutter diff-line__gutter--separator" />
          <span className="diff-line__content diff-line__content--separator">
            {line.content}
          </span>
        </div>
      );
    }
    
    const isHighlighted = highlightedLine === index;
    const lineClass = `diff-line diff-line--${line.type} ${isHighlighted ? 'diff-line--highlighted' : ''}`;
    
    const origNum = line.originalLineNumber ?? '';
    const modNum = line.modifiedLineNumber ?? '';

    const prefix = line.type === 'added' ? '+' : line.type === 'removed' ? '-' : ' ';

    return (
      <div
        key={`line-${index}`}
        className={lineClass}
        onClick={() => handleLineClick(index, line)}
      >
        {showLineNumbers && (
          lineNumberMode === 'single' ? (
            <span className="diff-line__gutter diff-line__gutter--single">
              <span className="diff-line__num">{index + 1}</span>
            </span>
          ) : (
            <span className="diff-line__gutter">
              <span className="diff-line__num diff-line__num--original">{origNum}</span>
              <span className="diff-line__num diff-line__num--modified">{modNum}</span>
            </span>
          )
        )}
        {showPrefix && <span className="diff-line__prefix">{prefix}</span>}
        <span className="diff-line__content">
          <SyntaxHighlighter
            language={detectedLanguage}
            style={customStyle}
            customStyle={{
              margin: 0,
              padding: 0,
              background: 'transparent',
              display: 'inline',
            }}
            codeTagProps={{
              style: {
                fontFamily: "'JetBrains Mono', 'Fira Code', 'SF Mono', 'Consolas', 'Monaco', 'Courier New', monospace",
                fontSize: '12px',
              }
            }}
            PreTag="span"
          >
            {line.content || ' '}
          </SyntaxHighlighter>
        </span>
      </div>
    );
  }, [detectedLanguage, showLineNumbers, lineNumberMode, showPrefix, highlightedLine, handleLineClick]);
  
  if (!originalContent && !modifiedContent) {
    return (
      <div className={`inline-diff-preview inline-diff-preview--empty ${className}`}>
        <span className="inline-diff-preview__placeholder">No content</span>
      </div>
    );
  }
  
  return (
    <div className={`inline-diff-preview ${className}`}>
      <div
        ref={containerRef}
        className="inline-diff-preview__content"
        style={{ maxHeight: `${maxHeight}px` }}
      >
        {diffLines.map((line, index) => renderLine(line, index))}
      </div>
    </div>
  );
});

InlineDiffPreview.displayName = 'InlineDiffPreview';

export default InlineDiffPreview;
