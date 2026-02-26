/**
 * Terminal output renderer based on xterm.js (read-only).
 * Uses TerminalActionManager to avoid per-instance EventBus listeners.
 */

import React, { useEffect, useRef, useCallback, memo, useId } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { registerTerminalActions, unregisterTerminalActions } from '../services/TerminalActionManager';
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

    const terminal = new XTerm({
      disableStdin: true,       // Disable input for read-only rendering.
      cursorBlink: false,
      cursorStyle: 'bar',
      cursorInactiveStyle: 'none',
      fontSize: 12,
      fontFamily: "'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace",
      lineHeight: 1.4,
      scrollback: 5000,
      convertEol: true,
      allowTransparency: true,
      theme: {
        background: 'transparent',
        foreground: '#d4d4d4',
        cursor: 'transparent',    // Hide cursor in read-only mode.
        cursorAccent: 'transparent',
        selection: 'rgba(255, 255, 255, 0.2)',
        black: '#1e1e1e',
        red: '#e06c75',
        green: '#98c379',
        yellow: '#e5c07b',
        blue: '#61afef',
        magenta: '#c678dd',
        cyan: '#56b6c2',
        white: '#abb2bf',
        brightBlack: '#5c6370',
        brightRed: '#e06c75',
        brightGreen: '#98c379',
        brightYellow: '#e5c07b',
        brightBlue: '#61afef',
        brightMagenta: '#c678dd',
        brightCyan: '#56b6c2',
        brightWhite: '#ffffff',
      },
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
      import('@/infrastructure/theme/core/ThemeService').then(({ themeService }) => {
        const theme = themeService.getCurrentTheme();
        const isDark = theme.type === 'dark';

        terminal.options.theme = {
          background: theme.colors.background.scene,
          foreground: theme.colors.text.primary,
          cursor: 'transparent',
          cursorAccent: theme.colors.background.secondary,
          selection: isDark ? 'rgba(255, 255, 255, 0.3)' : 'rgba(0, 0, 0, 0.3)',

          // Keep ANSI colors aligned with Terminal.tsx.
          black: '#000000',
          red: '#cd3131',
          green: '#0dbc79',
          yellow: '#e5e510',
          blue: '#2472c8',
          magenta: '#bc3fbc',
          cyan: '#11a8cd',
          white: isDark ? '#e5e5e5' : '#ffffff',
          brightBlack: '#666666',
          brightRed: '#f14c4c',
          brightGreen: '#23d18b',
          brightYellow: '#f5f543',
          brightBlue: '#3b8eea',
          brightMagenta: '#d670d6',
          brightCyan: '#29b8db',
          brightWhite: '#ffffff',
        };

        terminal.refresh(0, terminal.rows - 1);
      });
    };

    updateTheme();

    import('@/infrastructure/theme/core/ThemeService').then(({ themeService }) => {
      const unsubscribe = themeService.on('theme:after-change', updateTheme);

      return () => {
        unsubscribe?.();
      };
    });
  }, []);

  // Incremental write when content extends existing output.
  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) return;

    const lastContent = lastContentRef.current;
    
    if (content.startsWith(lastContent) && lastContent.length > 0) {
      const newPart = content.slice(lastContent.length);
      if (newPart) {
        terminal.write(newPart);
      }
    } else {
      terminal.clear();
      terminal.reset();
      if (content) {
        terminal.write(content);
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
