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
  private readonly onTick: () => void;
  private readonly intervalMs: number;
  private readonly scheduler: RemoteHeartbeatScheduler;
  private timerId: number = 0;

  constructor(
    onTick: () => void,
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
      this.onTick();
    }, this.intervalMs);
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
