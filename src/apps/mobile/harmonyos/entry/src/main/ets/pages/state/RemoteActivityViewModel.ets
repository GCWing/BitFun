import { SessionSummary } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from '../../services/ConnectionErrorPolicy';
import { AsyncLifecycleGate } from '../../services/AsyncLifecycleGate';
import { RemoteActivityLifecycleController } from '../../services/RemoteActivityLifecycleController';
import { RemoteConnectionCoordinator } from '../../services/RemoteConnectionCoordinator';

export class RemoteActivityViewModelHooks {
  readonly isConnected: () => boolean;
  readonly isBusy: () => boolean;
  readonly hasRemoteBinding: () => boolean;
  readonly isRemoteChat: () => boolean;
  readonly activeSession: () => SessionSummary;
  readonly onConnectionState: (state: string) => void;
  readonly onStatus: (status: string) => void;
  readonly onConnectionError: (err: Object) => Promise<boolean>;
  readonly onStopHeartbeat: () => void;
  readonly onStartPolling: () => void;
  readonly onStopPolling: () => void;
  readonly onPoll: () => Promise<void>;
  readonly onReconnect: () => Promise<void>;
  readonly onRestoreSession: (session: SessionSummary) => Promise<void>;

  constructor(
    isConnected: () => boolean,
    isBusy: () => boolean,
    hasRemoteBinding: () => boolean,
    isRemoteChat: () => boolean,
    activeSession: () => SessionSummary,
    onConnectionState: (state: string) => void,
    onStatus: (status: string) => void,
    onConnectionError: (err: Object) => Promise<boolean>,
    onStopHeartbeat: () => void,
    onStartPolling: () => void,
    onStopPolling: () => void,
    onPoll: () => Promise<void>,
    onReconnect: () => Promise<void>,
    onRestoreSession: (session: SessionSummary) => Promise<void>
  ) {
    this.isConnected = isConnected;
    this.isBusy = isBusy;
    this.hasRemoteBinding = hasRemoteBinding;
    this.isRemoteChat = isRemoteChat;
    this.activeSession = activeSession;
    this.onConnectionState = onConnectionState;
    this.onStatus = onStatus;
    this.onConnectionError = onConnectionError;
    this.onStopHeartbeat = onStopHeartbeat;
    this.onStartPolling = onStartPolling;
    this.onStopPolling = onStopPolling;
    this.onPoll = onPoll;
    this.onReconnect = onReconnect;
    this.onRestoreSession = onRestoreSession;
  }
}

/** Owns foreground recovery, heartbeat health checks, and idempotent resume cancellation. */
export class RemoteActivityViewModel {
  private readonly activity: RemoteActivityLifecycleController;
  private readonly connection: RemoteConnectionCoordinator;
  private readonly gate: AsyncLifecycleGate;
  private readonly hooks: RemoteActivityViewModelHooks;

  constructor(
    activity: RemoteActivityLifecycleController,
    connection: RemoteConnectionCoordinator,
    gate: AsyncLifecycleGate,
    hooks: RemoteActivityViewModelHooks
  ) {
    this.activity = activity;
    this.connection = connection;
    this.gate = gate;
    this.hooks = hooks;
  }

  startHeartbeat(): void {
    this.activity.startHeartbeat();
  }

  stopHeartbeat(): void {
    this.activity.stopHeartbeat();
  }

  async checkConnectionHealth(): Promise<void> {
    if (!this.hooks.isConnected() || this.hooks.isBusy()) {
      return;
    }
    try {
      await this.connection.ping();
    } catch (err) {
      if (await this.hooks.onConnectionError(err)) {
        return;
      }
      const message = ConnectionErrorPolicy.errorText(err);
      this.hooks.onConnectionState('failed');
      this.hooks.onStatus(message);
      this.hooks.onStopPolling();
      this.hooks.onStopHeartbeat();
    }
  }

  resume(): void {
    if (!this.hooks.hasRemoteBinding()) {
      return;
    }
    this.startHeartbeat();
    if (this.hooks.isRemoteChat() && this.hooks.activeSession().sessionId.length > 0) {
      this.hooks.onStartPolling();
      void this.hooks.onPoll();
    }
    void this.verifyForeground();
  }

  async verifyForeground(): Promise<void> {
    if (this.hooks.isBusy() || !this.hooks.hasRemoteBinding()) {
      return;
    }
    const token = this.gate.begin();
    try {
      this.hooks.onConnectionState('reconnecting');
      await this.connection.ping();
      if (!this.gate.isCurrent(token)) {
        return;
      }
      this.hooks.onConnectionState('connected');
      this.hooks.onStatus(RemoteI18n.t('connection.connected'));
      if (this.hooks.isRemoteChat() && this.hooks.activeSession().sessionId.length > 0) {
        await this.hooks.onPoll();
      }
    } catch (err) {
      if (!this.gate.isCurrent(token)) {
        return;
      }
      if (await this.hooks.onConnectionError(err)) {
        return;
      }
      this.hooks.onStatus(ConnectionErrorPolicy.errorText(err));
      this.hooks.onStopHeartbeat();
      this.hooks.onConnectionState('reconnecting');
      await this.reconnect(token);
    }
  }

  invalidate(): void {
    this.gate.invalidate();
    this.hooks.onStopPolling();
    this.stopHeartbeat();
  }

  private async reconnect(token: number): Promise<void> {
    const shouldRestore = this.hooks.isRemoteChat() && this.hooks.activeSession().sessionId.length > 0;
    const session = this.hooks.activeSession();
    await this.hooks.onReconnect();
    if (!this.gate.isCurrent(token) || !this.hooks.isConnected()) {
      return;
    }
    if (shouldRestore) {
      await this.hooks.onRestoreSession(session);
      this.hooks.onStartPolling();
      await this.hooks.onPoll();
    }
  }
}
