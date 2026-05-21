/**
 * CodePreview component
 * Lightweight, read-only code preview with syntax highlighting and streaming support
 *
 * Design notes:
 * 1. Use react-syntax-highlighter (Prism) instead of Monaco Editor
 *    - Monaco is heavy (2-3MB per instance) and hurts virtual list performance
 *    - Prism is lightweight and works well with streaming re-renders
 * 2. Auto-detect language from file extension
 * 3. Use memoization to avoid unnecessary re-renders
 * 4. Large content can be truncated when exceeding limits
 */

import React, { useMemo, memo, useRef, useEffect, useState, useCallback, useDeferredValue } from 'react';
import { getPrismLanguage } from '@/infrastructure/language-detection';
import { useTheme } from '@/infrastructure/theme';
import { getLoadedPrismSyntaxHighlighter, loadPrismSyntaxHighlighter } from '@/shared/utils/syntaxHighlighterLoader';
import { buildCodePreviewPrismStyle, CODE_PREVIEW_FONT_FAMILY } from './codePreviewPrismTheme';
import './CodePreview.scss';

export interface CodePreviewProps {
  /** Code content */
  content: string;
  /** File path (used for language detection and navigation) */
  filePath?: string;
  /** Explicit language (overrides auto-detection) */
  language?: string;
  /** Whether streaming is in progress */
  isStreaming?: boolean;
  /** Whether to show line numbers */
  showLineNumbers?: boolean;
  /** Custom class name */
  className?: string;
  /** Auto-scroll to bottom while streaming */
  autoScrollToBottom?: boolean;
  /** Max height (px) */
  maxHeight?: number;
  /** Line click callback (line numbers start at 1) */
  onLineClick?: (lineNumber: number, filePath?: string) => void;
}

/**
 * Detect language from file path using the global language detection service.
 */
function detectLanguageFromPath(filePath: string): string {
  if (!filePath) return 'text';
  return getPrismLanguage(filePath);
}

/**
 * CodePreview component with streaming-friendly syntax highlighting.
 */
export const CodePreview: React.FC<CodePreviewProps> = memo(({
  content,
  filePath,
  language,
  isStreaming = false,
  showLineNumbers = true,
  className = '',
  autoScrollToBottom = true,
  maxHeight = 400,
  onLineClick,
}) => {
  const { isLight } = useTheme();
  const prismStyle = useMemo(() => buildCodePreviewPrismStyle(isLight), [isLight]);
  const [SyntaxHighlighter, setSyntaxHighlighter] = useState<React.ComponentType<any> | null>(() => getLoadedPrismSyntaxHighlighter());

  const containerRef = useRef<HTMLDivElement>(null);
  const prevContentLengthRef = useRef(0);

  // During streaming, content updates at high frequency. Defer the highlighted
  // content passed to SyntaxHighlighter so that auto-scroll and cursor updates
  // (which use the real content) remain responsive on the main thread while
  // tokenization runs during browser idle time.
  const deferredContent = useDeferredValue(content);

  // Prism tokenizes the *entire* string synchronously. For large streaming files
  // (e.g. 500-line SCSS) this blocks the main thread for 50-150 ms per 100 ms
  // batch flush. Since the streaming preview shows at most 4 visible lines
  // (maxHeight ≈ 88 px), we only need to tokenize the tail of the buffer.
  // After streaming ends, the full content is restored for the completed view.
  const STREAMING_TAIL_LINES = 60; // generous tail – more than enough for any maxHeight
  const displayContentInfo = useMemo(() => {
    if (!isStreaming) {
      return { content: deferredContent, startingLineNumber: 1 };
    }

    const lines = deferredContent.split('\n');
    if (lines.length <= STREAMING_TAIL_LINES) {
      return { content: deferredContent, startingLineNumber: 1 };
    }

    const startingLineNumber = lines.length - STREAMING_TAIL_LINES + 1;
    return {
      content: lines.slice(-STREAMING_TAIL_LINES).join('\n'),
      startingLineNumber,
    };
  }, [isStreaming, deferredContent]);

  const displayContent = displayContentInfo.content;

  const [highlightedLine, setHighlightedLine] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    void loadPrismSyntaxHighlighter()
      .then((component) => {
        if (!cancelled) {
          setSyntaxHighlighter(() => component);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setSyntaxHighlighter(null);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);
  
  const detectedLanguage = useMemo(() => {
    if (language) return language;
    if (filePath) return detectLanguageFromPath(filePath);
    return 'text';
  }, [language, filePath]);
  
  // Auto-scroll only when content grows during streaming.
  useEffect(() => {
    if (!autoScrollToBottom || !isStreaming || !containerRef.current) return;
    
    if (content.length > prevContentLengthRef.current) {
      const container = containerRef.current;
      requestAnimationFrame(() => {
        container.scrollTop = container.scrollHeight;
      });
    }
    
    prevContentLengthRef.current = content.length;
  }, [content, isStreaming, autoScrollToBottom]);
  
  const handleLineClick = useCallback((lineNumber: number) => {
    setHighlightedLine(prev => prev === lineNumber ? null : lineNumber);
    // Trigger callback for editor navigation.
    onLineClick?.(lineNumber, filePath);
  }, [onLineClick, filePath]);
  
  const lineProps = useCallback((lineNumber: number): React.HTMLProps<HTMLElement> => {
    const actualLineNumber = displayContentInfo.startingLineNumber + lineNumber - 1;
    const isHighlighted = highlightedLine === actualLineNumber;
    return {
      style: {
        display: 'block',
        backgroundColor: isHighlighted ? 'rgba(99, 102, 241, 0.15)' : 'transparent',
        borderLeft: isHighlighted ? '3px solid var(--color-accent-500, #6366f1)' : '3px solid transparent',
        marginLeft: '-3px',
        paddingLeft: '3px',
        transition: 'background-color 0.15s ease, border-color 0.15s ease',
      },
      onClick: () => handleLineClick(actualLineNumber),
      className: isHighlighted ? 'code-line--highlighted' : '',
    };
  }, [highlightedLine, handleLineClick, displayContentInfo.startingLineNumber]);
  
  if (!content) {
    return (
      <div className={`code-preview code-preview--empty ${className}`}>
        <span className="code-preview__placeholder">No content</span>
      </div>
    );
  }
  
  const containerStyle: React.CSSProperties = {
    maxHeight: `${maxHeight}px`,
  };
  
  return (
    <div className={`code-preview ${isStreaming ? 'code-preview--streaming' : ''} ${className}`}>
      <div 
        ref={containerRef}
        className="code-preview__content"
        style={containerStyle}
      >
        {SyntaxHighlighter ? (
          <SyntaxHighlighter
            language={detectedLanguage}
            style={prismStyle}
            showLineNumbers={showLineNumbers}
            startingLineNumber={displayContentInfo.startingLineNumber}
            wrapLines={true}
            wrapLongLines={true}
            lineProps={lineProps}
            customStyle={{
              margin: 0,
              padding: 0,
              background: 'transparent',
              overflow: 'visible',
            }}
            codeTagProps={{
              style: {
                fontFamily: CODE_PREVIEW_FONT_FAMILY,
                fontSize: '12px',
                lineHeight: '1.6',
                fontWeight: 400,
              }
            }}
            lineNumberStyle={{
              minWidth: '2.5em',
              paddingRight: '1em',
              textAlign: 'right',
              userSelect: 'none',
              color: 'var(--color-text-muted, #666)',
              opacity: isLight ? 0.88 : 0.6,
            }}
          >
            {displayContent}
          </SyntaxHighlighter>
        ) : (
          <pre className="code-preview__plain" aria-label="Code preview">
            <code>
              {displayContent.split('\n').map((line, index) => {
                const lineNumber = displayContentInfo.startingLineNumber + index;
                return (
                  <span
                    key={`${lineNumber}-${index}`}
                    className={`code-preview__plain-line${highlightedLine === lineNumber ? ' code-preview__plain-line--highlighted' : ''}`}
                    onClick={() => handleLineClick(lineNumber)}
                  >
                    {showLineNumbers && (
                      <span className="code-preview__plain-line-number">{lineNumber}</span>
                    )}
                    <span className="code-preview__plain-line-content">{line || '\u00A0'}</span>
                  </span>
                );
              })}
            </code>
          </pre>
        )}
        
        {/* Streaming cursor indicator */}
        {isStreaming && (
          <span className="code-preview__cursor" />
        )}
      </div>
    </div>
  );
});

CodePreview.displayName = 'CodePreview';

export default CodePreview;

