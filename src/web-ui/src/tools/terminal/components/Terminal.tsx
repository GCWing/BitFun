/**
 * Terminal base component built on xterm.js.
 * Optimizations include debounced resize and visibility-aware refresh.
 */

import React, { useEffect, useRef, useCallback, useState, forwardRef, useImperativeHandle } from 'react';
import type { ITheme } from '@xterm/xterm';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { WebglAddon } from '@xterm/addon-webgl';
import {
  TerminalResizeDebouncer,
  buildXtermTheme,
  getXtermFontWeights,
  DEFAULT_XTERM_MINIMUM_CONTRAST_RATIO,
} from '../utils';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { themeService } from '@/infrastructure/theme/core/ThemeService';
import { createLogger } from '@/shared/utils/logger';
import '@xterm/xterm/css/xterm.css';
import './Terminal.scss';

const log = createLogger('Terminal');

type TerminalCoreWithMeasurement = XTerm & {
  _core?: {
    _charSizeService?: {
      measure?: () => void;
    };
    _renderService?: {
      handleDevicePixelRatioChange?: () => void;
    };
  };
};

/**
 * Clear xterm texture atlas when supported.
 * Used to force redraws and avoid WebGL cache artifacts.
 */
function clearTextureAtlas(terminal: XTerm): void {
  // clearTextureAtlas is internal; access via a type cast.
  const rawTerminal = terminal as unknown as { _core?: { _renderService?: { _renderer?: { _charAtlasCache?: { clear?: () => void }; clearTextureAtlas?: () => void } } } };
  try {
    rawTerminal._core?._renderService?._renderer?.clearTextureAtlas?.();
  } catch {
    // Ignore if unsupported.
  }
}

function remeasureTerminal(terminal: XTerm): void {
  const rawTerminal = terminal as TerminalCoreWithMeasurement;
  rawTerminal._core?._charSizeService?.measure?.();
  rawTerminal._core?._renderService?.handleDevicePixelRatioChange?.();
}

/**
 * Scroll to bottom when the cursor is below the viewport.
 */
function scrollToBottomIfNeeded(terminal: XTerm): void {
  const buffer = terminal.buffer.active;
  if (buffer.cursorY >= terminal.rows - 1) {
    terminal.scrollToBottom();
  }
}

export interface TerminalOptions {
  fontSize?: number;
  fontFamily?: string;
  lineHeight?: number;
  minimumContrastRatio?: number;
  cursorStyle?: 'block' | 'underline' | 'bar';
  cursorBlink?: boolean;
  scrollback?: number;
  /** Initial columns to avoid early wrapping. */
  cols?: number;
  rows?: number;
  theme?: {
    background?: string;
    foreground?: string;
    cursor?: string;
    cursorAccent?: string;
    selectionBackground?: string;
    selectionForeground?: string;
    selectionInactiveBackground?: string;
    black?: string;
    red?: string;
    green?: string;
    yellow?: string;
    blue?: string;
    magenta?: string;
    cyan?: string;
    white?: string;
    brightBlack?: string;
    brightRed?: string;
    brightGreen?: string;
    brightYellow?: string;
    brightBlue?: string;
    brightMagenta?: string;
    brightCyan?: string;
    brightWhite?: string;
  };
}

export interface TerminalProps {
  className?: string;
  /** For context menu identification. */
  terminalId?: string;
  /** For context menu identification. */
  sessionId?: string;
  options?: TerminalOptions;
  autoFocus?: boolean;
  onData?: (data: string) => void;
  onBinary?: (data: string) => void;
  onTitleChange?: (title: string) => void;
  /** Notify backend PTY about size changes. */
  onResize?: (cols: number, rows: number) => void;
  onReady?: (terminal: XTerm) => void;
  /**
   * Paste interceptor: return true to allow, false to block.
   * Uses the default multi-line confirmation when omitted.
   */
  onPaste?: (text: string) => Promise<boolean> | boolean;
  /**
   * When set to a positive value, doXtermResize skips any resize that would
   * shrink the terminal below this column count. Used during history replay to
   * prevent CSS-animation intermediate sizes from permanently truncating buffered
   * content. Set back to 0 (or leave unset) to restore normal resize behaviour.
   */
  preventShrinkBelowColsRef?: React.MutableRefObject<number>;
}

export interface TerminalRef {
  write: (data: string) => void;
  writeln: (data: string) => void;
  clear: () => void;
  reset: () => void;
  focus: () => void;
  fit: () => void;
  /** Flush pending debounced resize operations. */
  flushResize: () => void;
  /** Force a redraw (clears texture cache). */
  forceRedraw: () => void;
  getTerminal: () => XTerm | null;
  getSize: () => { cols: number; rows: number } | null;
}

/**
 * Build an xterm.js theme object from the current ThemeService state synchronously.
 * Calling this at XTerm construction time prevents the initial black-background flash
 * that occurs when the theme is applied asynchronously via useEffect.
 */
function getInitialXtermTheme(overrides: TerminalOptions['theme'] = {}): ITheme {
  return buildXtermTheme(themeService.getCurrentTheme(), overrides);
}

const DEFAULT_OPTIONS: TerminalOptions = {
  fontSize: 14,
  fontFamily: "'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace",
  lineHeight: 1.2,
  minimumContrastRatio: DEFAULT_XTERM_MINIMUM_CONTRAST_RATIO,
  cursorStyle: 'block',
  cursorBlink: true,
  scrollback: 10000,
};

const Terminal = forwardRef<TerminalRef, TerminalProps>(({
  className = '',
  terminalId,
  sessionId,
  options = {},
  autoFocus = false,
  onData,
  onBinary,
  onTitleChange,
  onResize,
  onReady,
  onPaste,
  preventShrinkBelowColsRef,
}, ref) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const webglAddonRef = useRef<WebglAddon | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const intersectionObserverRef = useRef<IntersectionObserver | null>(null);
  const resizeDebouncerRef = useRef<TerminalResizeDebouncer | null>(null);
  const isVisibleRef = useRef(true);
  const wasVisibleRef = useRef(false);
  const lastBackendSizeRef = useRef<{ cols: number; rows: number } | null>(null);
  const [isReady, setIsReady] = useState(false);
  const currentTheme = themeService.getCurrentTheme();
  const initialFontWeights = getXtermFontWeights(currentTheme.type);

  // Merge options. Theme is resolved from ThemeService at render time so that the
  // initial XTerm instance is created with the correct background color and avoids
  // the black-background flash that occurs when a light theme is active.
  const mergedOptions = {
    ...DEFAULT_OPTIONS,
    ...options,
    theme: {
      ...getInitialXtermTheme(),
      ...options.theme,
    },
  };

  // Force refresh for rendering consistency.
  const forceRefresh = useCallback((terminal: XTerm) => {
    const rows = terminal.rows;
    terminal.refresh(0, rows - 1);
    clearTextureAtlas(terminal);
  }, []);

  const doXtermResize = useCallback((cols: number, rows: number) => {
    const terminal = terminalRef.current;
    if (!terminal) return;

    try {
      if (terminal.cols === cols && terminal.rows === rows) {
        return;
      }

      // While the caller has set a minimum column guard (e.g., during history
      // replay), skip any resize that would shrink below that value.  This
      // prevents CSS open-animation intermediate widths from permanently
      // truncating buffered content that was written at a wider column count.
      const minCols = preventShrinkBelowColsRef?.current ?? 0;
      if (minCols > 0 && cols < minCols) {
        return;
      }

      terminal.resize(cols, rows);
    } catch (error) {
      log.warn('Xterm resize error', { cols, rows, error });
    }
  }, [preventShrinkBelowColsRef]);

  // Notify backend PTY with deduping.
  const doBackendResize = useCallback((cols: number, rows: number) => {
    const lastSize = lastBackendSizeRef.current;
    if (lastSize && lastSize.cols === cols && lastSize.rows === rows) {
      return;
    }
    
    lastBackendSizeRef.current = { cols, rows };
    
    onResize?.(cols, rows);
  }, [onResize]);

  // Post-resize fixups (refresh and cursor visibility).
  const handleResizeComplete = useCallback(() => {
    const terminal = terminalRef.current;
    if (!terminal) return;

    requestAnimationFrame(() => {
      if (terminalRef.current) {
        forceRefresh(terminalRef.current);
        scrollToBottomIfNeeded(terminalRef.current);
      }
    });
  }, [forceRefresh]);

  const fit = useCallback((immediate = false) => {
    if (!fitAddonRef.current || !terminalRef.current || !containerRef.current) {
      return;
    }

    try {
      const { clientWidth, clientHeight } = containerRef.current;
      if (clientWidth < 50 || clientHeight < 50) {
        return;
      }

      const dims = fitAddonRef.current.proposeDimensions();
      if (!dims || dims.cols <= 0 || dims.rows <= 0) {
        return;
      }

      // Skip tiny intermediate dimensions that occur when a panel CSS-animates
      // from zero width to its final size. xterm.js permanently truncates buffer
      // lines to the current column count on resize, so we must avoid resizing
      // to columns fewer than any content already in the buffer.
      // 40 cols is the minimum usable terminal width (below this, most shells
      // are unusable anyway and content would be permanently damaged).
      if (dims.cols < 40 || dims.rows < 3) {
        return;
      }

      if (resizeDebouncerRef.current) {
        resizeDebouncerRef.current.resize(dims.cols, dims.rows, immediate);
      } else {
        doXtermResize(dims.cols, dims.rows);
        doBackendResize(dims.cols, dims.rows);
        handleResizeComplete();
      }
    } catch (error) {
      log.warn('Fit error', error);
    }
  }, [doXtermResize, doBackendResize, handleResizeComplete]);

  const flushResize = useCallback(() => {
    resizeDebouncerRef.current?.flush();
  }, []);

  const forceRedraw = useCallback(() => {
    const terminal = terminalRef.current;
    if (terminal) {
      forceRefresh(terminal);
    }
  }, [forceRefresh]);

  useImperativeHandle(ref, () => ({
    write: (data: string) => {
      terminalRef.current?.write(data);
    },
    writeln: (data: string) => {
      terminalRef.current?.writeln(data);
    },
    clear: () => {
      terminalRef.current?.clear();
    },
    reset: () => {
      terminalRef.current?.reset();
    },
    focus: () => {
      terminalRef.current?.focus();
    },
    fit: () => fit(false),
    flushResize,
    forceRedraw,
    getTerminal: () => terminalRef.current,
    getSize: () => {
      if (terminalRef.current) {
        return {
          cols: terminalRef.current.cols,
          rows: terminalRef.current.rows,
        };
      }
      return null;
    },
  }), [fit, flushResize, forceRedraw]);

  useEffect(() => {
    if (!containerRef.current) return;

    // Let fit() determine size; backend starts at 80x24 and syncs via resize.
    const terminal = new XTerm({
      fontSize: mergedOptions.fontSize,
      fontFamily: mergedOptions.fontFamily,
      fontWeight: initialFontWeights.fontWeight,
      fontWeightBold: initialFontWeights.fontWeightBold,
      lineHeight: mergedOptions.lineHeight,
      minimumContrastRatio: mergedOptions.minimumContrastRatio,
      cursorStyle: mergedOptions.cursorStyle,
      cursorBlink: mergedOptions.cursorBlink,
      scrollback: mergedOptions.scrollback,
      theme: mergedOptions.theme,
      allowTransparency: true,
      // TUI apps usually handle line wrapping.
      convertEol: false,
    });

    const fitAddon = new FitAddon();
    // WebLinksAddon supports Ctrl+click to open URLs.
    let currentHoverTarget: HTMLElement | null = null;
    const webLinksAddon = new WebLinksAddon(
      (event, uri) => {
        if (event.ctrlKey) {
          systemAPI.openExternal(uri).catch((error) => {
            log.error('Failed to open external link', { uri, error });
          });
        }
      },
      {
        hover: (event, _uri, _range) => {
          const target = event.target as HTMLElement;
          if (target) {
            if (currentHoverTarget && currentHoverTarget !== target) {
              currentHoverTarget.removeAttribute('title');
            }
            currentHoverTarget = target;
            target.title = 'Ctrl + click to open link';
          }
        },
        leave: (event, _text) => {
          const target = event.target as HTMLElement;
          if (target) {
            target.removeAttribute('title');
          }
          if (currentHoverTarget) {
            currentHoverTarget.removeAttribute('title');
            currentHoverTarget = null;
          }
        },
      }
    );

    terminal.loadAddon(fitAddon);
    terminal.loadAddon(webLinksAddon);

    terminal.open(containerRef.current);

    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;

    // WebGL renderer must be loaded after terminal.open().
    try {
      const webglAddon = new WebglAddon();
      
      webglAddon.onContextLoss(() => {
        log.warn('WebGL context lost, falling back to canvas');
        webglAddon.dispose();
        webglAddonRef.current = null;
      });
      
      terminal.loadAddon(webglAddon);
      webglAddonRef.current = webglAddon;
    } catch (error) {
      log.debug('WebGL not available, using canvas', error);
    }

    const resizeDebouncer = new TerminalResizeDebouncer({
      getTerminal: () => terminalRef.current,
      isVisible: () => isVisibleRef.current,
      onXtermResize: doXtermResize,
      onBackendResize: doBackendResize,
      onFlush: () => {
        if (terminalRef.current) {
          forceRefresh(terminalRef.current);
        }
      },
      onResizeComplete: handleResizeComplete,
    });
    resizeDebouncerRef.current = resizeDebouncer;

    requestAnimationFrame(() => {
      fit(true);

      setIsReady(true);
      onReady?.(terminal);

      if (autoFocus) {
        terminal.focus();
      }
    });

    let fontLoadCancelled = false;
    if (typeof document !== 'undefined' && 'fonts' in document) {
      const fontSet = document.fonts as FontFaceSet;
      if (fontSet.status !== 'loaded') {
        void fontSet.ready.then(() => {
          if (fontLoadCancelled || !terminalRef.current) {
            return;
          }

          requestAnimationFrame(() => {
            if (!terminalRef.current) return;

            remeasureTerminal(terminalRef.current);
            fit(true);

            requestAnimationFrame(() => {
              if (!terminalRef.current) return;
              forceRefresh(terminalRef.current);
              scrollToBottomIfNeeded(terminalRef.current);
            });
          });
        });
      }
    }

    const dataDisposable = terminal.onData((data) => {
      onData?.(data);
    });

    const binaryDisposable = terminal.onBinary((data) => {
      onBinary?.(data);
    });

    const titleDisposable = terminal.onTitleChange((title) => {
      onTitleChange?.(title);
    });

    // Intercept paste (Ctrl+V / Ctrl+Shift+V).
    terminal.attachCustomKeyEventHandler((event: KeyboardEvent) => {
      if (event.type === 'keydown' && event.ctrlKey && (event.key === 'v' || event.key === 'V')) {
        event.preventDefault();
        
        (async () => {
          try {
            const text = await navigator.clipboard.readText();
            if (!text) return;

            if (onPaste) {
              const allowed = await onPaste(text);
              if (!allowed) {
                return;
              }
            }

            onData?.(text);
          } catch (err) {
            log.error('Paste failed', err);
          }
        })();
        
        return false;
      }
      
      return true;
    });

    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        fit(false);
      });
    });
    resizeObserver.observe(containerRef.current);
    resizeObserverRef.current = resizeObserver;

    // On visibility change, flush pending resize and refresh.
    const intersectionObserver = new IntersectionObserver((entries) => {
      const entry = entries[0];
      const isVisible = entry.isIntersecting;
      
      isVisibleRef.current = isVisible;

      if (isVisible && !wasVisibleRef.current) {
        requestAnimationFrame(() => {
          resizeDebouncerRef.current?.flush();
          
          fit(true);
          
          requestAnimationFrame(() => {
            const term = terminalRef.current;
            if (term) {
              term.refresh(0, term.rows - 1);
              clearTextureAtlas(term);
              scrollToBottomIfNeeded(term);
              if (autoFocus) {
                term.focus();
              }
            }
          });
        });
      }
      wasVisibleRef.current = isVisible;
    }, {
      threshold: 0.1
    });
    intersectionObserver.observe(containerRef.current);
    intersectionObserverRef.current = intersectionObserver;

    return () => {
      dataDisposable.dispose();
      binaryDisposable.dispose();
      titleDisposable.dispose();
      resizeObserver.disconnect();
      intersectionObserver.disconnect();
      fontLoadCancelled = true;
      resizeDebouncer.dispose();
      webglAddonRef.current?.dispose();
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
      webglAddonRef.current = null;
      resizeObserverRef.current = null;
      intersectionObserverRef.current = null;
      resizeDebouncerRef.current = null;
      lastBackendSizeRef.current = null;
    };
  }, []);

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal || !isReady) return;

    terminal.options.fontSize = mergedOptions.fontSize;
    terminal.options.fontFamily = mergedOptions.fontFamily;
    terminal.options.lineHeight = mergedOptions.lineHeight;
    terminal.options.minimumContrastRatio = mergedOptions.minimumContrastRatio;
    terminal.options.cursorStyle = mergedOptions.cursorStyle;
    terminal.options.cursorBlink = mergedOptions.cursorBlink;
    terminal.options.scrollback = mergedOptions.scrollback;
    terminal.options.theme = mergedOptions.theme;

    fit(true);
  }, [
    mergedOptions.fontSize,
    mergedOptions.fontFamily,
    mergedOptions.lineHeight,
    mergedOptions.minimumContrastRatio,
    mergedOptions.cursorStyle,
    mergedOptions.cursorBlink,
    mergedOptions.scrollback,
    isReady,
    fit,
  ]);

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal || !isReady) return;

    const updateXtermTheme = () => {
      (() => {
        const theme = themeService.getCurrentTheme();
        terminal.options.theme = buildXtermTheme(theme, options.theme);

        // Light-on-dark text appears bolder due to irradiation (optical illusion);
        // dark-on-light text looks thinner in comparison. Bump fontWeight in light
        // mode to compensate.
        const fontWeights = getXtermFontWeights(theme.type);
        terminal.options.fontWeight = fontWeights.fontWeight;
        terminal.options.fontWeightBold = fontWeights.fontWeightBold;

        forceRefresh(terminal);
      })();
    };

    updateXtermTheme();

    const unsubscribe = themeService.on('theme:after-change', updateXtermTheme);
    return () => {
      unsubscribe?.();
    };
  }, [isReady, forceRefresh, options.theme]);

  return (
    <div 
      className={`bitfun-terminal ${className}`}
      data-terminal-id={terminalId}
      data-session-id={sessionId}
    >
      <div 
        ref={containerRef} 
        className="bitfun-terminal__container"
      />
    </div>
  );
});

Terminal.displayName = 'Terminal';

export default Terminal;
