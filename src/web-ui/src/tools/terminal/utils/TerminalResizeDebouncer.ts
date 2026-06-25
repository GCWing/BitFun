/**
 * Terminal resize debouncer with split X/Y strategies and visibility-aware scheduling.
 */

import type { Terminal } from '@xterm/xterm';

/** Start debouncing when buffer rows exceed this threshold. */
const START_DEBOUNCING_THRESHOLD = 200;

/** Debounce delay for X-axis resize (ms). */
const RESIZE_X_DEBOUNCE_MS = 100;

/** Backend resize merge delay (ms). */
const BACKEND_RESIZE_DEBOUNCE_MS = 50;

/** Idle callback timeout (ms). */
const IDLE_CALLBACK_TIMEOUT_MS = 2000;

export interface ResizeCallback {
  (cols: number, rows: number): void | Promise<void>;
}

export interface ResizeDebounceOptions {
  getTerminal: () => Terminal | null;
  isVisible: () => boolean;
  /** Local xterm resize callback. Returns false when resize was intentionally skipped. */
  onXtermResize: (cols: number, rows: number) => boolean;
  /** Backend PTY resize callback (debounced/merged). */
  onBackendResize: (cols: number, rows: number) => void | Promise<void>;
  /** Optional hook invoked after flush. */
  onFlush?: () => void;
  /** Optional hook invoked after resize completes. */
  onResizeComplete?: () => void;
}

// Legacy options interface.
export interface LegacyResizeDebounceOptions {
  getTerminal: () => Terminal | null;
  isVisible: () => boolean;
  onResizeBoth: ResizeCallback;
  onResizeX: (cols: number) => void | Promise<void>;
  onResizeY: (rows: number) => void | Promise<void>;
  /** Optional hook invoked after flush. */
  onFlush?: () => void;
}

/** requestIdleCallback polyfill. */
const requestIdleCallback = 
  typeof window !== 'undefined' && 'requestIdleCallback' in window
    ? window.requestIdleCallback
    : (cb: IdleRequestCallback, options?: IdleRequestOptions) => {
        const start = Date.now();
        return window.setTimeout(() => {
          cb({
            didTimeout: false,
            timeRemaining: () => Math.max(0, 50 - (Date.now() - start)),
          });
        }, options?.timeout ?? 1) as unknown as number;
      };

const cancelIdleCallback =
  typeof window !== 'undefined' && 'cancelIdleCallback' in window
    ? window.cancelIdleCallback
    : (id: number) => clearTimeout(id);

/** Check whether options use the new API. */
function isNewOptions(options: ResizeDebounceOptions | LegacyResizeDebounceOptions): options is ResizeDebounceOptions {
  return 'onXtermResize' in options && 'onBackendResize' in options;
}

/**
 * Debounced terminal resize controller.
 * Uses immediate resize for small buffers, debounced backend updates, and idle scheduling when hidden.
 */
export class TerminalResizeDebouncer {
  private latestCols = 0;
  private latestRows = 0;
  
  private resizeXTimeoutId: ReturnType<typeof setTimeout> | null = null;
  private resizeXIdleCallbackId: number | null = null;
  private resizeYIdleCallbackId: number | null = null;
  
  private backendResizeTimeoutId: ReturnType<typeof setTimeout> | null = null;
  private pendingBackendResize: { cols: number; rows: number } | null = null;
  
  private readonly options: ResizeDebounceOptions | LegacyResizeDebounceOptions;
  private readonly isNewApi: boolean;
  private disposed = false;

  constructor(options: ResizeDebounceOptions | LegacyResizeDebounceOptions) {
    this.options = options;
    this.isNewApi = isNewOptions(options);
  }

  /**
   * Request a resize operation.
   */
  resize(cols: number, rows: number, immediate = false): void {
    if (this.disposed) return;

    this.latestCols = cols;
    this.latestRows = rows;

    const terminal = this.options.getTerminal();
    if (!terminal) return;

    const currentCols = terminal.cols;
    const currentRows = terminal.rows;

    if (cols === currentCols && rows === currentRows) {
      return;
    }

    const bufferLength = terminal.buffer.normal.length;

    if (immediate || bufferLength < START_DEBOUNCING_THRESHOLD) {
      this.clearPendingJobs();
      if (this.isNewApi) {
        const opts = this.options as ResizeDebounceOptions;
        // Backend resize is scheduled only after xterm accepted the same size.
        // This prevents PTY/frontend geometry drift when callers suppress a fit.
        if (opts.onXtermResize(cols, rows)) {
          this.scheduleBackendResize(cols, rows);
        }
      } else {
        this.executeResize(cols, rows, true);
      }
      return;
    }

    if (!this.options.isVisible()) {
      this.scheduleIdleResize(cols, rows);
      return;
    }

    const colsChanged = cols !== currentCols;
    const rowsChanged = rows !== currentRows;

    if (this.isNewApi) {
      const opts = this.options as ResizeDebounceOptions;
      
      if (rowsChanged) {
        // Row-only updates are still gated by local success; skipped local
        // resizes must not be mirrored to the PTY.
        if (opts.onXtermResize(currentCols, rows)) {
          this.scheduleBackendResize(currentCols, rows);
        }
      }
      if (colsChanged) {
        this.debounceXtermResizeX(cols, rows);
      }
    } else {
      const opts = this.options as LegacyResizeDebounceOptions;
      
      if (rowsChanged) {
        opts.onResizeY(rows);
      }

      if (colsChanged) {
        this.debounceResizeXLegacy(cols);
      }
    }
  }

  /**
   * Flush pending resize work.
   */
  flush(forceCallback = false): void {
    if (this.disposed) return;

    const hasPendingResize = 
      this.resizeXTimeoutId !== null ||
      this.resizeXIdleCallbackId !== null ||
      this.resizeYIdleCallbackId !== null ||
      this.backendResizeTimeoutId !== null;

    if (hasPendingResize) {
      this.clearPendingJobs();
      this.executeResize(this.latestCols, this.latestRows, true);
    }
    
    if ((hasPendingResize || forceCallback) && this.options.onFlush) {
      this.options.onFlush();
    }
  }

  /**
   * Get the latest target size.
   */
  getLatestSize(): { cols: number; rows: number } {
    return {
      cols: this.latestCols,
      rows: this.latestRows,
    };
  }

  hasPendingResize(): boolean {
    return (
      this.resizeXTimeoutId !== null ||
      this.resizeXIdleCallbackId !== null ||
      this.resizeYIdleCallbackId !== null ||
      this.backendResizeTimeoutId !== null
    );
  }

  dispose(): void {
    this.disposed = true;
    this.clearPendingJobs();
  }

  private executeResize(cols: number, rows: number, includeBackend: boolean): void {
    if (this.isNewApi) {
      const opts = this.options as ResizeDebounceOptions;
      const resized = opts.onXtermResize(cols, rows);
      if (includeBackend) {
        // Flush can run after a suspended resize. Respect the local result so a
        // stale pending target cannot resize only the backend.
        if (resized) {
          opts.onBackendResize(cols, rows);
          opts.onResizeComplete?.();
        }
      }
    } else {
      const opts = this.options as LegacyResizeDebounceOptions;
      opts.onResizeBoth(cols, rows);
    }
  }

  private debounceXtermResizeX(cols: number, rows: number): void {
    if (this.resizeXTimeoutId !== null) {
      clearTimeout(this.resizeXTimeoutId);
    }

    this.resizeXTimeoutId = setTimeout(() => {
      this.resizeXTimeoutId = null;
      if (!this.disposed && this.isNewApi) {
        const opts = this.options as ResizeDebounceOptions;
        if (opts.onXtermResize(cols, rows)) {
          this.scheduleBackendResize(cols, rows);
        }
      }
    }, RESIZE_X_DEBOUNCE_MS);
  }

  private scheduleBackendResize(cols: number, rows: number): void {
    if (!this.isNewApi) return;
    
    this.pendingBackendResize = { cols, rows };
    
    if (this.backendResizeTimeoutId !== null) {
      return;
    }
    
    this.backendResizeTimeoutId = setTimeout(() => {
      this.backendResizeTimeoutId = null;
      if (!this.disposed && this.pendingBackendResize) {
        const opts = this.options as ResizeDebounceOptions;
        const { cols: c, rows: r } = this.pendingBackendResize;
        this.pendingBackendResize = null;
        opts.onBackendResize(c, r);
        opts.onResizeComplete?.();
      }
    }, BACKEND_RESIZE_DEBOUNCE_MS);
  }

  private debounceResizeXLegacy(cols: number): void {
    if (this.resizeXTimeoutId !== null) {
      clearTimeout(this.resizeXTimeoutId);
    }

    this.resizeXTimeoutId = setTimeout(() => {
      this.resizeXTimeoutId = null;
      if (!this.disposed && !this.isNewApi) {
        const opts = this.options as LegacyResizeDebounceOptions;
        opts.onResizeX(cols);
      }
    }, RESIZE_X_DEBOUNCE_MS);
  }

  private scheduleIdleResize(cols: number, rows: number): void {
    const terminal = this.options.getTerminal();
    if (!terminal) return;

    const currentCols = terminal.cols;
    const currentRows = terminal.rows;

    if (this.isNewApi) {
      if (this.resizeXIdleCallbackId === null) {
        this.resizeXIdleCallbackId = requestIdleCallback(
          () => {
            this.resizeXIdleCallbackId = null;
            if (!this.disposed) {
              this.executeResize(this.latestCols, this.latestRows, true);
            }
          },
          { timeout: IDLE_CALLBACK_TIMEOUT_MS }
        );
      }
    } else {
      const opts = this.options as LegacyResizeDebounceOptions;
      
      if (cols !== currentCols && this.resizeXIdleCallbackId === null) {
        this.resizeXIdleCallbackId = requestIdleCallback(
          () => {
            this.resizeXIdleCallbackId = null;
            if (!this.disposed) {
              opts.onResizeX(this.latestCols);
            }
          },
          { timeout: IDLE_CALLBACK_TIMEOUT_MS }
        );
      }

      if (rows !== currentRows && this.resizeYIdleCallbackId === null) {
        this.resizeYIdleCallbackId = requestIdleCallback(
          () => {
            this.resizeYIdleCallbackId = null;
            if (!this.disposed) {
              opts.onResizeY(this.latestRows);
            }
          },
          { timeout: IDLE_CALLBACK_TIMEOUT_MS }
        );
      }
    }
  }

  private clearPendingJobs(): void {
    if (this.resizeXTimeoutId !== null) {
      clearTimeout(this.resizeXTimeoutId);
      this.resizeXTimeoutId = null;
    }

    if (this.resizeXIdleCallbackId !== null) {
      cancelIdleCallback(this.resizeXIdleCallbackId);
      this.resizeXIdleCallbackId = null;
    }

    if (this.resizeYIdleCallbackId !== null) {
      cancelIdleCallback(this.resizeYIdleCallbackId);
      this.resizeYIdleCallbackId = null;
    }

    if (this.backendResizeTimeoutId !== null) {
      clearTimeout(this.backendResizeTimeoutId);
      this.backendResizeTimeoutId = null;
      this.pendingBackendResize = null;
    }
  }
}

export default TerminalResizeDebouncer;
