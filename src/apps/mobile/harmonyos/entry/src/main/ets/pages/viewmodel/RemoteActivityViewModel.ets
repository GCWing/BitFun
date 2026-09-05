import { SessionSummary } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from '../../services/ConnectionErrorPolicy';
import { AsyncLifecycleGate } from '../../services/AsyncLifecycleGate';
import { RemoteActivityLifecycleController } from '../../services/RemoteActivityLifecycleController';
import { RemoteConnectionCoordinator } from '../../services/RemoteConnectionCoordinator';
import { RemoteLogger } from '../../services/RemoteLogger';

export interface RemoteActivityViewModelHooks {
  readonly isConnected: () => boolean;
  readonly isReconnecting: () => boolean;
  readonly isBusy: () => boolean;
  readonly hasRemoteBinding: () => boolean;
  readonly isRemoteChat: () => boolean;
  readonly activeSession: () => SessionSummary;
  readonly onConnectionState: (state: string) => void;
  readonly onStatus: (status: string) => void;
  readonly onConnectionError: (err: Object) => Promise<boolean>;
  readonly onStartPolling: () => void;
  readonly onStopPolling: () => void;
  readonly onPoll: () => Promise<void>;
  readonly onReconnect: () => Promise<void>;
  readonly onRestoreSession: (session: SessionSummary) => Promise<void>;
}

/** Owns foreground recovery, heartbeat health checks, and idempotent resume cancellation. */
export class RemoteActivityViewModel {
  private readonly activity: RemoteActivityLifecycleController;
  private readonly connection: RemoteConnectionCoordinator;
  private readonly gate: AsyncLifecycleGate;
  private readonly hooks: RemoteActivityViewModelHooks;
  private recovering: boolean = false;

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

  /**
   * A health check while the link is up, a recovery probe once it is not.
   *
   * A failed ping used to park the connection in `failed` and stop the
   * heartbeat with it. Nothing survived that: the poller was stopped too, and
   * `failed` disables every remote action, so a desktop that blinked out for a
   * moment left the phone dead until the user backgrounded and refocused the
   * app. The heartbeat is the only thing still ticking at that point, which
   * makes it the only thing that can notice the desktop coming back — so it
   * keeps ticking, and the first ping that answers puts the session back on the
   * wire. Ticks are serialised by the heartbeat itself, and a ping is cheap
   * enough to repeat every interval for as long as the drop lasts.
   */
  async checkConnectionHealth(): Promise<void> {
    if (this.hooks.isBusy() || this.recovering || !this.hooks.hasRemoteBinding()) {
      return;
    }
    const wasConnected = this.hooks.isConnected();
    if (!wasConnected && !this.hooks.isReconnecting()) {
      return;
    }
    if (!wasConnected) {
      const token = this.gate.begin();
      this.recovering = true;
      try {
        await this.reconnect(token);
      } catch (err) {
        if (this.gate.isCurrent(token)) {
          this.hooks.onStatus(ConnectionErrorPolicy.errorText(err));
        }
      } finally {
        this.recovering = false;
      }
      return;
    }
    try {
      await this.connection.ping();
    } catch (err) {
      if (await this.hooks.onConnectionError(err)) {
        return;
      }
      this.hooks.onStatus(ConnectionErrorPolicy.errorText(err));
      if (wasConnected) {
        RemoteLogger.info('heartbeat ping failed, waiting for the link to come back');
        this.hooks.onStopPolling();
        this.hooks.onConnectionState('reconnecting');
      }
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
      this.hooks.onConnectionState('reconnecting');
      // The heartbeat keeps running through this. If the reconnect below fails
      // there is nothing else left to try again, and its next tick is the retry
      // — it just has to stay out of the way while this one is in flight.
      this.recovering = true;
      try {
        await this.reconnect(token);
      } catch (reconnectErr) {
        this.hooks.onStatus(ConnectionErrorPolicy.errorText(reconnectErr));
      } finally {
        this.recovering = false;
      }
    }
  }

  invalidate(): void {
    this.gate.invalidate();
    this.hooks.onStopPolling();
    this.stopHeartbeat();
  }

  /** Keeps only the active-turn poll alive during a transient background window. */
  suspendKeepingActiveTurnPoll(): void {
    this.gate.invalidate();
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
