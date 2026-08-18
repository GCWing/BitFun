import { RemoteLogger } from './RemoteLogger';

const TICK_MS = 250;
const REPORT_THRESHOLD_MS = 400;

/**
 * Tells a blocked main thread apart from a deferred frame.
 *
 * A fixed-interval timer whose own lateness is the measurement: the tick is
 * trivial, so anything it arrives late by was spent by somebody else holding
 * the JS thread. Without this, a log line that lands 1.6s after the state
 * write behind it reads the same either way — the thread was busy, or ArkUI
 * simply had not painted yet — and those two want opposite fixes.
 *
 * Armed for a window rather than left running, so nothing pays 4Hz for a
 * measurement that only matters around an event someone asked about.
 */
export class MainThreadStallProbe {
  private static timerId: number = 0;
  private static deadline: number = 0;
  private static expectedAt: number = 0;
  private static reason: string = '';

  /** Watches for `windowMs`, restarting the window if one is already open. */
  static watch(reason: string, windowMs: number): void {
    MainThreadStallProbe.reason = reason;
    MainThreadStallProbe.deadline = Date.now() + windowMs;
    if (MainThreadStallProbe.timerId !== 0) {
      return;
    }
    MainThreadStallProbe.arm();
  }

  static stop(): void {
    if (MainThreadStallProbe.timerId !== 0) {
      clearTimeout(MainThreadStallProbe.timerId);
      MainThreadStallProbe.timerId = 0;
    }
    MainThreadStallProbe.deadline = 0;
  }

  private static arm(): void {
    MainThreadStallProbe.expectedAt = Date.now() + TICK_MS;
    MainThreadStallProbe.timerId = setTimeout(() => {
      MainThreadStallProbe.timerId = 0;
      const now = Date.now();
      const late = now - MainThreadStallProbe.expectedAt;
      if (late >= REPORT_THRESHOLD_MS) {
        RemoteLogger.info(`main thread stall reason=${MainThreadStallProbe.reason} blocked_ms=${late}`);
      }
      if (now >= MainThreadStallProbe.deadline) {
        MainThreadStallProbe.deadline = 0;
        return;
      }
      MainThreadStallProbe.arm();
    }, TICK_MS);
  }
}
