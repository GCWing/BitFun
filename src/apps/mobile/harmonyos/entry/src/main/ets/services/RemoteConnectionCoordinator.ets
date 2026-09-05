import { InitialSyncResult } from '../model/RemoteModels';
import { MobileIdentityStore } from './MobileIdentityStore';
import { RemoteDescriptorParser } from './RemoteDescriptorParser';
import { RemotePairingPolicy } from './RemotePairingPolicy';
import { RemoteSessionManager } from './RemoteSessionManager';
import { AsyncLifecycleGate } from './AsyncLifecycleGate';
import { RemoteLogger } from './RemoteLogger';

export interface RemoteConnectionRequest {
  remoteUrl: string;
  userId: string;
  deviceId: string;
  accountPassword: string;
  autoReconnect: boolean;
}

/** Owns remote protocol setup; UI state is returned to the page as a result. */
export class RemoteConnectionCoordinator {
  private readonly sessionManager: RemoteSessionManager;
  private readonly identityStore: MobileIdentityStore;
  private readonly pairingPolicy: RemotePairingPolicy;
  private readonly lifecycleGate: AsyncLifecycleGate;
  private activeToken: number = 0;

  constructor(
    sessionManager: RemoteSessionManager,
    identityStore: MobileIdentityStore,
    pairingPolicy: RemotePairingPolicy,
    lifecycleGate: AsyncLifecycleGate
  ) {
    this.sessionManager = sessionManager;
    this.identityStore = identityStore;
    this.pairingPolicy = pairingPolicy;
    this.lifecycleGate = lifecycleGate;
  }

  async connect(request: RemoteConnectionRequest): Promise<InitialSyncResult> {
    const token: number = this.lifecycleGate.begin();
    this.activeToken = token;
    const descriptor = RemoteDescriptorParser.parse(request.remoteUrl);
    const identity = this.pairingPolicy.identityForConnect(
      descriptor,
      request.userId,
      request.deviceId,
      request.accountPassword,
      request.autoReconnect
    );
    const initialSync: InitialSyncResult = await this.sessionManager.connect(
      descriptor,
      identity,
      request.deviceId
    );
    if (!this.lifecycleGate.isCurrent(token)) {
      throw new Error('Remote connection was invalidated');
    }
    try {
      await this.identityStore.savePairingInput(identity.userId, request.remoteUrl);
      await this.identityStore.clearUserIdProtection();
    } catch (err) {
      // Pairing has already succeeded. Keep the live channel usable and make
      // the loss of restart durability explicit in logs instead of reporting
      // the whole connection as failed.
      RemoteLogger.warn(`pairing snapshot persistence failed: ${String(err)}`);
    }
    if (!this.lifecycleGate.isCurrent(token)) {
      throw new Error('Remote connection was invalidated');
    }
    return initialSync;
  }

  invalidate(): void {
    this.lifecycleGate.invalidate();
  }

  isCurrentRequest(): boolean {
    return this.activeToken > 0 && this.lifecycleGate.isCurrent(this.activeToken);
  }

  async ping(): Promise<void> {
    await this.sessionManager.ping();
  }

  reset(): void {
    this.invalidate();
    this.sessionManager.reset();
  }
}
