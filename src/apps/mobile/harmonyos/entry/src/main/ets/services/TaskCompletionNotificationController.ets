export type TaskCompletionSource = 'remote' | 'general';

export interface TaskCompletionObservation {
  source: TaskCompletionSource;
  sessionId: string;
  turnId: string;
}

export interface TaskCompletionNotificationPort {
  prepare(): Promise<void>;
  keepAlive(onExpired: () => void): boolean;
  releaseKeepAlive(): void;
  publish(observation: TaskCompletionObservation): Promise<void>;
}

export type TaskCompletionObservationExpired = (observation: TaskCompletionObservation) => void;

/**
 * Owns the foreground/background completion-notification state machine.
 *
 * Platform notification and background-task APIs stay behind the injected
 * port so lifecycle policy remains deterministic and unit-testable.
 */
export class TaskCompletionNotificationController {
  private readonly port: TaskCompletionNotificationPort;
  private readonly onExpired: TaskCompletionObservationExpired;
  private observation?: TaskCompletionObservation;
  private backgrounded: boolean = false;

  constructor(
    port: TaskCompletionNotificationPort,
    onExpired: TaskCompletionObservationExpired = (_observation: TaskCompletionObservation): void => {}
  ) {
    this.port = port;
    this.onExpired = onExpired;
  }

  track(observation: TaskCompletionObservation): void {
    const sessionId = observation.sessionId.trim();
    const turnId = observation.turnId.trim();
    if (sessionId.length === 0 || turnId.length === 0) {
      return;
    }
    if (this.backgrounded && this.observation && this.observation.sessionId !== sessionId) {
      return;
    }
    this.observation = {
      source: observation.source,
      sessionId,
      turnId
    };
    void this.port.prepare();
  }

  onBackground(): boolean {
    if (!this.observation) {
      return false;
    }
    this.backgrounded = true;
    const observation = this.observation;
    const keptAlive = this.port.keepAlive((): void => {
      const expired = this.observation;
      if (!this.backgrounded || !expired) {
        return;
      }
      this.backgrounded = false;
      this.observation = undefined;
      this.onExpired(expired);
    });
    if (!keptAlive) {
      this.backgrounded = false;
      this.observation = undefined;
      this.onExpired(observation);
      return false;
    }
    return true;
  }

  onForeground(): void {
    this.backgrounded = false;
    this.observation = undefined;
    this.port.releaseKeepAlive();
  }

  complete(source: TaskCompletionSource, sessionId: string, turnId: string): boolean {
    const observation = this.observation;
    if (!observation || observation.source !== source ||
      observation.sessionId !== sessionId.trim() || observation.turnId !== turnId.trim()) {
      return false;
    }
    const shouldPublish = this.backgrounded;
    this.backgrounded = false;
    this.observation = undefined;
    this.port.releaseKeepAlive();
    if (shouldPublish) {
      void this.port.publish(observation);
    }
    return shouldPublish;
  }

  /** Clears a failed or cancelled observation and reports whether it was backgrounded. */
  cancel(source: TaskCompletionSource, sessionId: string, turnId: string = ''): boolean {
    const observation = this.observation;
    const expectedTurnId = turnId.trim();
    if (!observation || observation.source !== source || observation.sessionId !== sessionId.trim() ||
      (expectedTurnId.length > 0 && observation.turnId !== expectedTurnId)) {
      return false;
    }
    const wasBackgrounded = this.backgrounded;
    this.backgrounded = false;
    this.observation = undefined;
    this.port.releaseKeepAlive();
    return wasBackgrounded;
  }

  stop(): void {
    this.onForeground();
  }

  isWatchingInBackground(): boolean {
    return this.backgrounded && this.observation !== undefined;
  }

  isTracking(source: TaskCompletionSource): boolean {
    return this.observation !== undefined && this.observation.source === source;
  }

}
