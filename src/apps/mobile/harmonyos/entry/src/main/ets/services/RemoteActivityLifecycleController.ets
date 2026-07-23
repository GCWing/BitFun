import { RemoteHeartbeatController, RemoteHeartbeatScheduler } from './RemoteHeartbeatController';

export class RemoteActivityLifecycleController {
  private readonly heartbeatController: RemoteHeartbeatController;

  constructor(
    onHeartbeatTick: () => void,
    heartbeatIntervalMs: number = 15000,
    heartbeatScheduler?: RemoteHeartbeatScheduler
  ) {
    this.heartbeatController = new RemoteHeartbeatController(
      onHeartbeatTick,
      heartbeatIntervalMs,
      heartbeatScheduler
    );
  }

  startHeartbeat(): void {
    this.heartbeatController.start();
  }

  stopHeartbeat(): void {
    this.heartbeatController.stop();
  }

  isHeartbeatRunning(): boolean {
    return this.heartbeatController.isRunning();
  }
}
