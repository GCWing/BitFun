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

import React, { useEffect, useRef, useCallback, memo, useId } from 'react';
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
}

/**
 * xterm.js read-only output renderer.
 */
export const TerminalOutputRenderer: React.FC<TerminalOutputRendererProps> = memo(({
  content,
  className = '',
  terminalId: propTerminalId,
  minHeight = 60,
  maxHeight = 300,
}) => {
  const autoId = useId();
  const terminalId = propTerminalId || `terminal-output-${autoId}`;
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const lastContentRef = useRef<string>('');

  // Estimate height from content.
  const calculateHeight = useCallback((text: string): number => {
    if (!text) return minHeight;
    
    const lines = text.split('\n');
    const lineHeight = 18;
    const estimatedHeight = lines.length * lineHeight + 16;
    
    return Math.min(Math.max(estimatedHeight, minHeight), maxHeight);
  }, [minHeight, maxHeight]);

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
        fitAddon.fit();
      } catch {
        // Ignore fit errors.
      }
    });

    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        try {
          fitAddon.fit();
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
  }, []);

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

    const lastContent = lastContentRef.current;
    
    // Compare raw content for incremental detection, but replace absolute
    // cursor-position sequences with CR+LF before writing so a fresh xterm.js
    // context does not show blank rows caused by ESC[row;colH jumps, while
    // keeping the line boundary between the sections they separated.
    if (content.startsWith(lastContent) && lastContent.length > 0) {
      const newPart = content.slice(lastContent.length);
      if (newPart) {
        terminal.write(normalizeAbsoluteCursorPositions(newPart));
      }
    } else {
      terminal.clear();
      terminal.reset();
      if (content) {
        terminal.write(normalizeAbsoluteCursorPositions(content));
      }
    }
    
    lastContentRef.current = content;

    requestAnimationFrame(() => {
      try {
        fitAddonRef.current?.fit();
      } catch {
        // Ignore fit errors.
      }
    });
  }, [content]);

  const height = calculateHeight(content);

  return (
    <div 
      ref={containerRef}
      className={`terminal-output-renderer ${className}`}
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

TerminalOutputRenderer.displayName = 'TerminalOutputRenderer';

export default TerminalOutputRenderer;
