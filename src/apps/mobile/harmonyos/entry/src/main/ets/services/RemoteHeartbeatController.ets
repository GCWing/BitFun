export interface RemoteHeartbeatScheduler {
  setInterval(callback: () => void, intervalMs: number): number;
  clearInterval(timerId: number): void;
}

class SystemRemoteHeartbeatScheduler implements RemoteHeartbeatScheduler {
  setInterval(callback: () => void, intervalMs: number): number {
    return setInterval(callback, intervalMs);
  }

  clearInterval(timerId: number): void {
    clearInterval(timerId);
  }
}

export class RemoteHeartbeatController {
  private readonly onTick: () => Promise<void>;
  private readonly intervalMs: number;
  private readonly scheduler: RemoteHeartbeatScheduler;
  private timerId: number = 0;
  private inFlight: boolean = false;

  constructor(
    onTick: () => Promise<void>,
    intervalMs: number = 15000,
    scheduler: RemoteHeartbeatScheduler = new SystemRemoteHeartbeatScheduler()
  ) {
    this.onTick = onTick;
    this.intervalMs = intervalMs;
    this.scheduler = scheduler;
  }

  start(): void {
    this.stop();
    this.timerId = this.scheduler.setInterval(() => {
      void this.runTick();
    }, this.intervalMs);
  }

  /**
   * A tick that arrives while the previous one is still out is dropped, not
   * queued. The interval counts wall-clock time, but a heartbeat can outlive
   * many intervals: a desktop that has stopped answering holds every request
   * until the relay's own timeout, two minutes later. Starting one more each
   * interval turns a single unanswered request into a pile of them aimed at the
   * desktop that is already not answering.
   */
  private async runTick(): Promise<void> {
    if (this.inFlight) {
      return;
    }
    this.inFlight = true;
    try {
      await this.onTick();
    } finally {
      this.inFlight = false;
    }
  }

  stop(): void {
    if (this.timerId !== 0) {
      this.scheduler.clearInterval(this.timerId);
      this.timerId = 0;
    }
  }

  isRunning(): boolean {
    return this.timerId !== 0;
  }
}
