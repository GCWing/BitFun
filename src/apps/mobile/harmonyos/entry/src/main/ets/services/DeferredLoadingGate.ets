/**
 * Holds a loading state back until waiting for it is something the user can
 * actually perceive.
 *
 * Opening a session that is already cached needs one short read off disk, and
 * announcing that with a full-pane skeleton costs more than it explains: the
 * transcript is replaced by a placeholder and then by itself again, which reads
 * as a stall rather than as speed. Below the grace period nothing is shown at
 * all; past it the read is slow enough — a fetch over the relay, say — that
 * silence would be the worse answer.
 *
 * Each `arm` supersedes the one before it, so a second tap while the first open
 * is still settling cannot have its skeleton dismissed by the first one
 * finishing. The token is what enforces that.
 */
export class DeferredLoadingGate {
  private readonly delayMs: number;
  private readonly onVisible: (visible: boolean) => void;
  private timerId: number = 0;
  private token: number = 0;

  constructor(delayMs: number, onVisible: (visible: boolean) => void) {
    this.delayMs = delayMs;
    this.onVisible = onVisible;
  }

  /** Starts a wait. The returned token is what `release` has to be given. */
  arm(): number {
    this.cancelTimer();
    this.token += 1;
    const armed = this.token;
    this.timerId = setTimeout(() => {
      this.timerId = 0;
      if (armed !== this.token) {
        return;
      }
      this.onVisible(true);
    }, this.delayMs);
    return armed;
  }

  /** The token of the most recent `arm`, for callers keeping state beside it. */
  currentToken(): number {
    return this.token;
  }

  /** Ends the wait started by `token`, if it is still the current one. */
  release(token: number): void {
    if (token !== this.token) {
      return;
    }
    this.cancelTimer();
    this.onVisible(false);
  }

  /** Ends whatever wait is open, for teardown. */
  reset(): void {
    this.cancelTimer();
    this.token += 1;
    this.onVisible(false);
  }

  private cancelTimer(): void {
    if (this.timerId !== 0) {
      clearTimeout(this.timerId);
      this.timerId = 0;
    }
  }
}
