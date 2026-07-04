/**
 * InlineDiffPreview component.
 * Lightweight inline diff preview for tool cards.
 *
 * Design notes:
 * 1. Avoid Monaco DiffEditor (too heavy)
 * 2. Use the diff library (npm: diff) for performance
 * 3. GitHub-style unified diff view
 * 4. Token-first syntax highlighting: tokenize full content once via prismjs,
 *    split into per-line token arrays, render without per-line SyntaxHighlighter instances
 * 5. Row virtualization via @tanstack/react-virtual: only visible rows are in the DOM
 */

import React, { useMemo, memo, useRef, useCallback, useState, useLayoutEffect, CSSProperties } from 'react';
import Prism from 'prismjs';
import { useVirtualizer } from '@tanstack/react-virtual';
import { diffLines, Change } from 'diff';
import { getPrismLanguage } from '@/infrastructure/language-detection';
import { useTheme } from '@/infrastructure/theme';
import { createLogger } from '@/shared/utils/logger';
import { buildCodePreviewPrismStyle, CODE_PREVIEW_FONT_FAMILY } from './codePreviewPrismTheme';
import './InlineDiffPreview.scss';

const log = createLogger('InlineDiffPreview');

/** Estimated row height in px — must match CSS line-height × font-size. */
const ROW_HEIGHT = 22;

/**
 * Maximum total lines (original + modified) before content is truncated.
 * Keeps diffLines() + Prism.tokenize() cost bounded for large files.
 * The caller may also pre-truncate; this is a safety net inside the component.
 */
const MAX_TOTAL_LINES = 500;
/** Character budget that complements MAX_TOTAL_LINES for very long single lines. */
const MAX_TOTAL_CHARS = 50_000;

/** Result of truncation: possibly shortened content + metadata. */
interface TruncationResult {
  originalContent: string;
  modifiedContent: string;
  truncated: boolean;
  omittedLines: number;
}

/**
 * Truncate original/modified content when combined line or char count exceeds
 * thresholds. Returns head + tail so both ends of the diff remain visible.
 */
function truncateForDiff(original: string, modified: string): TruncationResult {
  const origLines = original ? original.split('\n') : [];
  const modLines = modified ? modified.split('\n') : [];
  const totalLines = origLines.length + modLines.length;
  const totalChars = original.length + modified.length;

  if (totalLines <= MAX_TOTAL_LINES && totalChars <= MAX_TOTAL_CHARS) {
  return { originalContent: original, modifiedContent: modified, truncated: false, omittedLines: 0 };
  }

  // Budget per side, per half (head/tail).
  const perSide = Math.max(50, Math.floor(MAX_TOTAL_LINES / 4));

  const slice = (lines: string[]): { text: string; kept: number; dropped: number } => {
    if (lines.length <= perSide * 2) return { text: lines.join('\n'), kept: lines.length, dropped: 0 };
    const head = lines.slice(0, perSide);
    const tail = lines.slice(lines.length - perSide);
    return {
      text: [...head, '', `... truncated ${lines.length - perSide * 2} lines ...`, '', ...tail].join('\n'),
      kept: perSide * 2,
      dropped: lines.length - perSide * 2,
    };
  };

  const o = slice(origLines);
  const m = slice(modLines);
  return {
    originalContent: o.text,
    modifiedContent: m.text,
    truncated: true,
    omittedLines: o.dropped + m.dropped,
  };
}

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

/** A single token from Prism. */
type PrismToken = string | Prism.Token;

/** Per-line token array after splitting the full-content token stream. */
type LineTokens = PrismToken[];

// ---------------------------------------------------------------------------
// Tokenization utilities
// ---------------------------------------------------------------------------

/**
 * Split a flat Prism token stream into per-line arrays.
 * Handles nested Token.content recursively.
 */
function splitTokensByNewlines(tokens: PrismToken[]): LineTokens[] {
  const lines: LineTokens[] = [[]];

  function walk(token: PrismToken): void {
    if (typeof token === 'string') {
      const parts = token.split('\n');
      for (let i = 0; i < parts.length; i++) {
        if (i > 0) lines.push([]);
        if (parts[i] !== '') lines[lines.length - 1].push(parts[i]);
      }
    } else {
      // Prism.Token with content that may be nested
      const { type, content, alias } = token;
      if (Array.isArray(content)) {
        // Collect child tokens into a sub-array, then rewrap with correct type
        const before = lines.length;
        const startIdx = lines[lines.length - 1].length;

        for (const child of content) walk(child);

        // If all child tokens stayed on the same original line, merge them back
        // into a single token to keep the structure compact. Otherwise the
        // content already ended up split across lines — leave as-is.
        if (lines.length === before) {
          // Same line: replace what we appended with a re-wrapped token
          const children = lines[lines.length - 1].splice(startIdx);
          if (children.length > 0) {
            lines[lines.length - 1].push(new Prism.Token(type, children, alias));
          }
        }
      } else if (typeof content === 'string') {
        const parts = content.split('\n');
        for (let i = 0; i < parts.length; i++) {
          if (i > 0) lines.push([]);
          if (parts[i] !== '') {
            lines[lines.length - 1].push(new Prism.Token(type, parts[i], alias));
          }
        }
      }
    }
  }

  for (const token of tokens) walk(token);
  return lines;
}

/**
 * Tokenize a full content string with prismjs, return per-line token arrays.
 * Falls back to plain-text lines when the language grammar is not registered.
 */
function tokenizeContent(content: string, language: string): LineTokens[] {
  if (!content) return [];
  const grammar = Prism.languages[language];
  if (!grammar) {
    // Graceful fallback: split by lines, no highlighting
    return content.split('\n').map(line => [line]);
  }
  try {
    const tokens = Prism.tokenize(content, grammar);
    return splitTokensByNewlines(tokens);
  } catch {
    return content.split('\n').map(line => [line]);
  }
}

/**
 * Render a single Prism token as a React element.
 * Mirrors what react-syntax-highlighter does internally.
 */
function renderToken(
  token: PrismToken,
  stylesheet: Record<string, CSSProperties>,
  key: string | number,
): React.ReactNode {
  if (typeof token === 'string') return token;

  const aliases = Array.isArray(token.alias) ? token.alias : token.alias ? [token.alias] : [];
  const classNames = ['token', token.type, ...aliases];
  const style: CSSProperties = classNames.reduce<CSSProperties>((acc, cls) => {
    return { ...acc, ...(stylesheet[`.${cls}`] ?? stylesheet[cls] ?? {}) };
  }, {});

  const children = Array.isArray(token.content)
    ? (token.content as PrismToken[]).map((child, i) => renderToken(child, stylesheet, i))
    : typeof token.content === 'string'
    ? token.content
    : null;

  return (
    <span key={key} className={classNames.join(' ')} style={style}>
      {children}
    </span>
  );
}

/**
 * Render a line's token array as React children.
 */
function renderTokenLine(tokens: LineTokens, stylesheet: Record<string, CSSProperties>): React.ReactNode {
  if (!tokens || tokens.length === 0) return '\u00A0'; // non-breaking space for empty lines
  return tokens.map((token, i) => renderToken(token, stylesheet, i));
}

// ---------------------------------------------------------------------------
// Diff computation (unchanged from original)
// ---------------------------------------------------------------------------

function computeLineDiff(originalContent: string, modifiedContent: string): DiffLine[] {
  const result: DiffLine[] = [];

  const changes: Change[] = diffLines(originalContent, modifiedContent);

  let originalLineNumber = 1;
  let modifiedLineNumber = 1;

  for (const change of changes) {
    const lines = change.value.split('\n');
    if (lines.length > 0 && lines[lines.length - 1] === '') {
      lines.pop();
    }

    for (const line of lines) {
      if (change.added) {
        result.push({ type: 'added', content: line, modifiedLineNumber: modifiedLineNumber++ });
      } else if (change.removed) {
        result.push({ type: 'removed', content: line, originalLineNumber: originalLineNumber++ });
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

function applyContextCollapsing(lines: DiffLine[], contextLines: number): DiffLine[] {
  if (contextLines < 0) return lines;

  const changeIndices: number[] = [];
  lines.forEach((line, index) => {
    if (line.type === 'added' || line.type === 'removed') changeIndices.push(index);
  });

  if (changeIndices.length === 0) {
    return [{ type: 'context-separator', content: 'No differences; contents are identical.' }];
  }

  const showLine = new Set<number>();
  for (const idx of changeIndices) {
    for (let i = Math.max(0, idx - contextLines); i <= Math.min(lines.length - 1, idx + contextLines); i++) {
      showLine.add(i);
    }
  }

  const result: DiffLine[] = [];
  let lastShownIndex = -1;

  for (let i = 0; i < lines.length; i++) {
    if (showLine.has(i)) {
      if (lastShownIndex >= 0 && i > lastShownIndex + 1) {
        result.push({ type: 'context-separator', content: `... omitted ${i - lastShownIndex - 1} lines ...` });
      }
      result.push(lines[i]);
      lastShownIndex = i;
    }
  }

  if (result.length > 0 && result[0].type !== 'context-separator') {
    const firstShownIdx = Array.from(showLine).sort((a, b) => a - b)[0];
    if (firstShownIdx > 0) {
      result.unshift({ type: 'context-separator', content: `... omitted first ${firstShownIdx} lines ...` });
    }
  }

  if (lastShownIndex < lines.length - 1) {
    result.push({
      type: 'context-separator',
      content: `... omitted last ${lines.length - 1 - lastShownIndex} lines ...`,
    });
  }

  return result;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/**
 * InlineDiffPreview component.
 *
 * Performance model:
 *   - Tokenize originalContent once  (useMemo)
 *   - Tokenize modifiedContent once  (useMemo)
 *   - Virtualize rows: only ~6 DOM nodes regardless of total line count
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
  const { isLight } = useTheme();
  const prismStyle = useMemo(() => buildCodePreviewPrismStyle(isLight), [isLight]);

  const containerRef = useRef<HTMLDivElement>(null);
  const [highlightedLine, setHighlightedLine] = useState<number | null>(null);

  const detectedLanguage = useMemo(() => {
    if (language) return language;
    if (filePath) return getPrismLanguage(filePath);
    return 'text';
  }, [language, filePath]);

  // Truncate very large inputs before diff/tokenization to protect the main thread.
  const truncated = useMemo(
    () => truncateForDiff(originalContent, modifiedContent),
    [originalContent, modifiedContent],
  );

  // Compute diff line list (fast, O(ND))
  const diffLineList = useMemo(() => {
    try {
      const rawDiff = computeLineDiff(truncated.originalContent, truncated.modifiedContent);
      return applyContextCollapsing(rawDiff, contextLines);
    } catch (error) {
      log.error('Diff computation failed', error);
      return [{ type: 'context-separator' as const, content: 'Diff computation failed; file may be too large.' }];
    }
  }, [truncated.originalContent, truncated.modifiedContent, contextLines]);

  // Tokenize each content once — O(content_length), not O(lines²)
  const originalLineTokens = useMemo(
    () => tokenizeContent(truncated.originalContent, detectedLanguage),
    [truncated.originalContent, detectedLanguage],
  );
  const modifiedLineTokens = useMemo(
    () => tokenizeContent(truncated.modifiedContent, detectedLanguage),
    [truncated.modifiedContent, detectedLanguage],
  );

  // Build stylesheet from prism style for token coloring
  const stylesheet = useMemo<Record<string, CSSProperties>>(() => {
    // prismStyle keys are CSS selectors like "token comment", ".token.comment", etc.
    // Normalize to ".className" → CSSProperties for renderToken lookup.
    const map: Record<string, CSSProperties> = {};
    for (const [selector, styles] of Object.entries(prismStyle)) {
      // e.g. "token comment" → entries for "token" and "comment"
      const parts = selector.split(/\s+|\./).filter(Boolean);
      for (const part of parts) {
        if (part && !map[part]) map[part] = styles as CSSProperties;
        if (part && !map[`.${part}`]) map[`.${part}`] = styles as CSSProperties;
      }
    }
    return map;
  }, [prismStyle]);

  // Line number → token array lookup helpers
  const getTokensForLine = useCallback(
    (line: DiffLine): LineTokens => {
      if (line.type === 'removed') {
        const idx = (line.originalLineNumber ?? 1) - 1;
        return originalLineTokens[idx] ?? [line.content];
      }
      if (line.type === 'added' || line.type === 'unchanged') {
        const idx = (line.modifiedLineNumber ?? 1) - 1;
        return modifiedLineTokens[idx] ?? [line.content];
      }
      return [line.content];
    },
    [originalLineTokens, modifiedLineTokens],
  );

  // Virtualizer with dynamic measurement so wrapped long lines get correct height.
  const virtualizer = useVirtualizer({
    count: diffLineList.length,
    getScrollElement: () => containerRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 3,
    measureElement: (el) => el.getBoundingClientRect().height,
  });

  // Re-measure all rows when the container width changes (wrapping may differ).
  // We use a generation counter to force re-renders WITHOUT calling
  // virtualizer.measure() — measure() resets the measurements cache and
  // would discard heights already captured by measureElement refs.
  const [measureGeneration, setMeasureGeneration] = useState(0);
  useLayoutEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    // On mount, after measureElement refs have stored actual heights,
    // force a synchronous re-render so getVirtualItems() uses those
    // measurements instead of the 22 px estimates.  Without this,
    // wrapped long lines (common in .md files) cause rows to overlap
    // because their translateY positions are based on wrong estimates.
    setMeasureGeneration((g) => g + 1);

    const ro = new ResizeObserver(() => {
      // Container width changed — wrapping may differ; re-render so the
      // virtualizer picks up the new measureElement heights.
      setMeasureGeneration((g) => g + 1);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const handleLineClick = useCallback(
    (index: number, line: DiffLine) => {
      if (line.type === 'context-separator') return;
      setHighlightedLine(prev => (prev === index ? null : index));
      if (onLineClick) {
        const lineNum = line.type === 'removed' ? line.originalLineNumber : line.modifiedLineNumber;
        const type = line.type === 'removed' ? 'original' : 'modified';
        if (lineNum) onLineClick(lineNum, type);
      }
    },
    [onLineClick],
  );

  if (!originalContent && !modifiedContent) {
    return (
      <div className={`inline-diff-preview inline-diff-preview--empty ${className}`}>
        <span className="inline-diff-preview__placeholder">No content</span>
      </div>
    );
  }

  const totalHeight = virtualizer.getTotalSize();
  const virtualItems = virtualizer.getVirtualItems();

  // measureGeneration is a render-trigger counter (bumped by useLayoutEffect
  // and ResizeObserver).  Referencing it here proves to TypeScript that the
  // state is consumed; the virtualizer's getTotalSize / getVirtualItems are
  // called unconditionally above and naturally pick up the measurements that
  // measureElement refs store before the first paint.
  void measureGeneration;

  return (
    <div className={`inline-diff-preview ${className}`}>
      {truncated.truncated && (
        <div className="inline-diff-preview__truncation-notice">
          Content too large; showing first and last portions ({truncated.omittedLines} lines omitted).
        </div>
      )}
      <div
        ref={containerRef}
        className="inline-diff-preview__content"
        style={{ maxHeight: `${maxHeight}px`, overflow: 'auto' }}
      >
        {/* Spacer div that gives the scrollable area its full virtual height */}
        <div style={{ height: totalHeight, width: '100%', position: 'relative' }}>
          {virtualItems.map(virtualRow => {
            const line = diffLineList[virtualRow.index];
            const isHighlighted = highlightedLine === virtualRow.index;

            if (line.type === 'context-separator') {
              return (
                <div
                  key={virtualRow.key}
                  ref={virtualizer.measureElement}
                  className="diff-line diff-line--separator"
                  data-index={virtualRow.index}
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                >
                  <span className="diff-line__gutter diff-line__gutter--separator" />
                  <span className="diff-line__content diff-line__content--separator">{line.content}</span>
                </div>
              );
            }

            const lineClass = [
              'diff-line',
              `diff-line--${line.type}`,
              isHighlighted ? 'diff-line--highlighted' : '',
            ]
              .filter(Boolean)
              .join(' ');

            const origNum = line.originalLineNumber ?? '';
            const modNum = line.modifiedLineNumber ?? '';
            const prefix = line.type === 'added' ? '+' : line.type === 'removed' ? '-' : ' ';
            const lineTokens = getTokensForLine(line);

            return (
              <div
                key={virtualRow.key}
                ref={virtualizer.measureElement}
                className={lineClass}
                data-index={virtualRow.index}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  transform: `translateY(${virtualRow.start}px)`,
                }}
                onClick={() => handleLineClick(virtualRow.index, line)}
              >
                {showLineNumbers &&
                  (lineNumberMode === 'single' ? (
                    <span className="diff-line__gutter diff-line__gutter--single">
                      <span className="diff-line__num">{virtualRow.index + 1}</span>
                    </span>
                  ) : (
                    <span className="diff-line__gutter">
                      <span className="diff-line__num diff-line__num--original">{origNum}</span>
                      <span className="diff-line__num diff-line__num--modified">{modNum}</span>
                    </span>
                  ))}
                {showPrefix && <span className="diff-line__prefix">{prefix}</span>}
                <span
                  className="diff-line__content"
                  style={{ fontFamily: CODE_PREVIEW_FONT_FAMILY, fontSize: '12px', fontWeight: 400 }}
                >
                  {renderTokenLine(lineTokens, stylesheet)}
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
});

InlineDiffPreview.displayName = 'InlineDiffPreview';

export default InlineDiffPreview;
