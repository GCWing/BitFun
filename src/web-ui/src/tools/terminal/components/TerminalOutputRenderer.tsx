/**
 * Terminal output renderer based on xterm.js (read-only).
 * Uses TerminalActionManager to avoid per-instance EventBus listeners.
 *
 * Raw PTY output may contain absolute cursor-position sequences (ESC[row;colH)
 * that assume existing content on screen.  When replayed in a fresh xterm.js
 * these sequences leave blank rows at the top.  We strip them before writing
 * so content flows sequentially; colors and relative movements are preserved.
 */

/**
 * Normalize absolute cursor-position sequences for fresh-context rendering.
 *
 * ESC[row;colH (CUP) and ESC[row;colf (HVP) reposition the cursor to an
 * absolute screen coordinate.  In a live terminal the rows above that
 * coordinate already contain shell prompts and prior output, so no blank space
 * appears.  In a fresh xterm.js context those rows are empty, producing a
 * large blank area before the first line of real content.
 *
 * We replace each such sequence with CR+LF so the two sections it separates
 * stay on different lines (plain deletion would cause them to run together),
 * while avoiding the blank-row artifact from coordinate-based positioning.
 *
 * Colors, bold, relative cursor movements and all other sequences are left
 * untouched.
 */
function normalizeAbsoluteCursorPositions(content: string): string {
  // Matches ESC [ <optional digits> ; <optional digits> H|f
  // e.g. ESC[14;35H  ESC[18;1H  ESC[5;1H  ESC[H  ESC[;1H
  // eslint-disable-next-line no-control-regex -- ESC sequences are intentional terminal control codes.
  return content.replace(/\x1b\[\d*;?\d*[Hf]/g, '\r\n');
}

function trimTrailingLineBreaksBeforeAnsiTail(content: string): string {
  // A final newline moves the xterm cursor to an extra blank row, which can
  // push useful content into scrollback in compact read-only previews. Preserve
  // trailing CSI state-reset sequences such as ESC[?25h while dropping only the
  // blank line break before them.
  // eslint-disable-next-line no-control-regex -- ESC sequences are intentional terminal control codes.
  return content.replace(/(?:\r\n|\r|\n)+((?:\x1b\[[0-?]*[ -/]*[@-~])*)$/g, '$1');
}

function prepareReadOnlyTerminalOutput(content: string): string {
  return trimTrailingLineBreaksBeforeAnsiTail(
    normalizeAbsoluteCursorPositions(content),
  );
}

import { forwardRef, memo, useCallback, useEffect, useId, useImperativeHandle, useRef, useState } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { registerTerminalActions, unregisterTerminalActions } from '../services/TerminalActionManager';
import { themeService } from '@/infrastructure/theme/core/ThemeService';
import {
  buildXtermTheme,
  getXtermFontWeights,
  DEFAULT_XTERM_MINIMUM_CONTRAST_RATIO,
} from '../utils';
import '@xterm/xterm/css/xterm.css';

const OUTPUT_FONT_SIZE = 12;
const OUTPUT_LINE_HEIGHT = 1.4;
const FALLBACK_OUTPUT_ROW_HEIGHT = Math.ceil(OUTPUT_FONT_SIZE * OUTPUT_LINE_HEIGHT);

interface TerminalOutputRendererProps {
  /** Output content to render. */
  content: string;
  /** Optional class name. */
  className?: string;
  /** Terminal ID for context menus; auto-generated if omitted. */
  terminalId?: string;
  /** Minimum height. */
  minHeight?: number;
  /** Maximum height. */
  maxHeight?: number;
  /** Maximum visible terminal rows. Takes precedence over maxHeight. */
  maxRows?: number;
}

export interface TerminalOutputRendererHandle {
  getVisibleText: () => string;
}

function getTerminalVisibleText(terminal: XTerm | null): string {
  if (!terminal) {
    return '';
  }

  const buffer = terminal.buffer.active;
  const lines: string[] = [];
  const endRow = Math.min(buffer.viewportY + terminal.rows, buffer.length);

  for (let row = buffer.viewportY; row < endRow; row += 1) {
    lines.push(buffer.getLine(row)?.translateToString(true) ?? '');
  }

  while (lines.length > 0 && !lines[lines.length - 1].trim()) {
    lines.pop();
  }

  return lines.join('\n');
}

function hasScrollableTerminalBuffer(terminal: XTerm | null): boolean {
  const buffer = terminal?.buffer.active;
  if (!terminal || !buffer) {
    return false;
  }

  return buffer.baseY > 0 || buffer.length > terminal.rows;
}

/**
 * xterm.js read-only output renderer.
 */
const TerminalOutputRendererComponent = forwardRef<TerminalOutputRendererHandle, TerminalOutputRendererProps>(({
  content,
  className = '',
  terminalId: propTerminalId,
  minHeight = FALLBACK_OUTPUT_ROW_HEIGHT,
  maxHeight = 300,
  maxRows,
}, ref) => {
  const autoId = useId();
  const terminalId = propTerminalId || `terminal-output-${autoId}`;
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const lastRenderedContentRef = useRef<string>('');
  const [rowHeight, setRowHeight] = useState(FALLBACK_OUTPUT_ROW_HEIGHT);
  const [hasScrollableBuffer, setHasScrollableBuffer] = useState(false);
  const preparedContent = prepareReadOnlyTerminalOutput(content);

  useImperativeHandle(ref, () => ({
    getVisibleText: () => getTerminalVisibleText(terminalRef.current),
  }), []);

  const heightForRows = useCallback((rows: number): number => {
    return Math.ceil(Math.max(rowHeight, rows * rowHeight));
  }, [rowHeight]);

  const alignHeightToRows = useCallback((height: number, mode: 'floor' | 'ceil'): number => {
    const rows = mode === 'ceil'
      ? Math.ceil(height / rowHeight)
      : Math.floor(height / rowHeight);
    return heightForRows(Math.max(1, rows));
  }, [heightForRows, rowHeight]);

  // Estimate height from content, keeping the container aligned to full xterm rows.
  const calculateHeight = useCallback((text: string): number => {
    const effectiveMinHeight = alignHeightToRows(minHeight, 'ceil');
    const effectiveMaxHeight = maxRows != null
      ? heightForRows(maxRows)
      : alignHeightToRows(maxHeight, 'floor');
    const boundedMaxHeight = Math.max(effectiveMinHeight, effectiveMaxHeight);

    if (!text) return Math.min(effectiveMinHeight, boundedMaxHeight);

    const lines = text.split(/\r\n|\r|\n/);
    const visibleRows = maxRows != null
      ? Math.min(lines.length, maxRows)
      : lines.length;
    const estimatedHeight = heightForRows(Math.max(1, visibleRows));
    
    return Math.min(Math.max(estimatedHeight, effectiveMinHeight), boundedMaxHeight);
  }, [alignHeightToRows, heightForRows, maxHeight, maxRows, minHeight]);

  const height = calculateHeight(preparedContent);
  const updateScrollableBufferState = useCallback(() => {
    setHasScrollableBuffer(hasScrollableTerminalBuffer(terminalRef.current));
  }, []);

  useEffect(() => {
    if (!containerRef.current) return;

    const currentTheme = themeService.getCurrentTheme();
    const fontWeights = getXtermFontWeights(currentTheme.type);
    const terminal = new XTerm({
      disableStdin: true,       // Disable input for read-only rendering.
      cursorBlink: false,
      cursorStyle: 'bar',
      cursorInactiveStyle: 'none',
      fontSize: 12,
      fontFamily: "'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace",
      fontWeight: fontWeights.fontWeight,
      fontWeightBold: fontWeights.fontWeightBold,
      lineHeight: 1.4,
      minimumContrastRatio: DEFAULT_XTERM_MINIMUM_CONTRAST_RATIO,
      scrollback: 5000,
      convertEol: true,
      allowTransparency: false,
      theme: buildXtermTheme(currentTheme, {
        cursor: 'transparent',    // Hide cursor in read-only mode.
        cursorAccent: 'transparent',
      }),
    });

    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(containerRef.current);

    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;

    requestAnimationFrame(() => {
      try {
        const nextRowHeight = terminal.dimensions?.css.cell.height;
        if (typeof nextRowHeight === 'number' && nextRowHeight > 0) {
          setRowHeight(nextRowHeight);
        }
        fitAddon.fit();
        updateScrollableBufferState();
      } catch {
        // Ignore fit errors.
      }
    });

    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        try {
          const nextRowHeight = terminal.dimensions?.css.cell.height;
          if (typeof nextRowHeight === 'number' && nextRowHeight > 0) {
            setRowHeight(nextRowHeight);
          }
          fitAddon.fit();
          updateScrollableBufferState();
        } catch {
          // Ignore fit errors.
        }
      });
    });
    resizeObserver.observe(containerRef.current);
    resizeObserverRef.current = resizeObserver;

    return () => {
      resizeObserver.disconnect();
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
      resizeObserverRef.current = null;
    };
  }, [updateScrollableBufferState]);

  // Register with TerminalActionManager to avoid per-instance EventBus listeners.
  useEffect(() => {
    registerTerminalActions(terminalId, {
      getTerminal: () => terminalRef.current,
      isReadOnly: true,
    });

    return () => {
      unregisterTerminalActions(terminalId);
    };
  }, [terminalId]);

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) return;

    const updateTheme = () => {
      const theme = themeService.getCurrentTheme();
      const fontWeights = getXtermFontWeights(theme.type);

      terminal.options.theme = buildXtermTheme(theme, {
        cursor: 'transparent',
        cursorAccent: 'transparent',
      });
      terminal.options.fontWeight = fontWeights.fontWeight;
      terminal.options.fontWeightBold = fontWeights.fontWeightBold;
      terminal.refresh(0, terminal.rows - 1);
    };

    updateTheme();

    const unsubscribe = themeService.on('theme:after-change', updateTheme);
    return () => {
      unsubscribe?.();
    };
  }, []);

  // Incremental write when content extends existing output.
  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) return;

    const lastRenderedContent = lastRenderedContentRef.current;
    
    // Compare the normalized read-only text for incremental detection so the
    // height estimate and xterm buffer receive the same content.
    if (preparedContent.startsWith(lastRenderedContent) && lastRenderedContent.length > 0) {
      const newPart = preparedContent.slice(lastRenderedContent.length);
      if (newPart) {
        terminal.write(newPart);
      }
    } else {
      terminal.clear();
      terminal.reset();
      if (preparedContent) {
        terminal.write(preparedContent);
      }
    }
    updateScrollableBufferState();
    
    lastRenderedContentRef.current = preparedContent;

    requestAnimationFrame(() => {
      try {
        fitAddonRef.current?.fit();
        updateScrollableBufferState();
      } catch {
        // Ignore fit errors.
      }
    });
  }, [preparedContent, updateScrollableBufferState]);

  return (
    <div 
      ref={containerRef}
      className={`terminal-output-renderer ${className} ${hasScrollableBuffer ? 'terminal-output-renderer--scrollable' : 'terminal-output-renderer--no-scroll'}`}
      data-terminal-id={terminalId}
      data-readonly="true"
      style={{
        height: `${height}px`,
        width: '100%',
        overflow: 'hidden',
      }}
    />
  );
});

TerminalOutputRendererComponent.displayName = 'TerminalOutputRenderer';

export const TerminalOutputRenderer = memo(TerminalOutputRendererComponent);

export default TerminalOutputRenderer;
