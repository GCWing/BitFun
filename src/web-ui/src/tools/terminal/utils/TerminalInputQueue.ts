/**
 * Coalesces terminal input into batched, sequentially-ordered writes.
 *
 * Problem: xterm.js fires `onData` once per keystroke. If each call triggers a
 * separate fire-and-forget `invoke('terminal_write')`, rapid typing creates a
 * burst of concurrent IPC calls. On macOS (WKWebView) this can cause character
 * loss because:
 *  - concurrent async command handlers are not guaranteed FIFO ordering after
 *    the `get_or_init_api` mutex releases,
 *  - WKWebView IPC has higher latency and bursts of concurrent invokes can
 *    interfere with each other.
 *
 * Solution: buffer incoming data and flush it as a single batched write. Only
 * one flush is in flight at a time, so ordering is guaranteed and IPC overhead
 * is reduced. The first keystroke is dispatched via a microtask (near-zero
 * latency); subsequent keystrokes that arrive while a flush is in flight are
 * automatically coalesced into the next flush.
 *
 * IME safety: xterm.js fires `onData` only after IME composition completes, so
 * this queue never interferes with input-method candidate selection or the
 * Enter-key guard in chat input components.
 */
export class TerminalInputQueue {
  private buffer = '';
  private flushing = false;
  private readonly write: (data: string) => Promise<void>;
  private readonly onError: (error: unknown) => void;

  constructor(
    write: (data: string) => Promise<void>,
    onError: (error: unknown) => void,
  ) {
    this.write = write;
    this.onError = onError;
  }

  /**
   * Append data to the buffer and schedule a flush if one is not already in
   * progress.
   */
  enqueue(data: string): void {
    this.buffer += data;
    if (!this.flushing) {
      this.flushing = true;
      queueMicrotask(() => {
        void this.drain();
      });
    }
  }

  /**
   * Flush the current buffer contents to the write function. After the write
   * completes, if more data arrived during the flush, drain again; otherwise
   * mark the queue as idle.
   */
  private async drain(): Promise<void> {
    const data = this.buffer;
    this.buffer = '';
    try {
      if (data) {
        await this.write(data);
      }
    } catch (err) {
      this.onError(err);
    } finally {
      if (this.buffer) {
        // More data arrived during the flush — drain again.
        void this.drain();
      } else {
        this.flushing = false;
      }
    }
  }

  /**
   * Discard any buffered data that has not been flushed yet. Data that is
   * already mid-flush is unaffected.
   */
  clear(): void {
    this.buffer = '';
  }
}
