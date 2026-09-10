import { RemoteSessionManager } from './RemoteSessionManager';
import { AsyncLifecycleGate } from './AsyncLifecycleGate';

/** Connection lifetime shared by account selection and the activity monitor. */
export class RemoteConnectionCoordinator {
  private readonly sessionManager: RemoteSessionManager;
  private readonly lifecycleGate: AsyncLifecycleGate;

  constructor(sessionManager: RemoteSessionManager, lifecycleGate: AsyncLifecycleGate) {
    this.sessionManager = sessionManager;
    this.lifecycleGate = lifecycleGate;
  }

  invalidate(): void { this.lifecycleGate.invalidate(); }
  async ping(): Promise<void> { await this.sessionManager.ping(); }
  reset(): void {
    this.invalidate();
    this.sessionManager.reset();
  }
}
