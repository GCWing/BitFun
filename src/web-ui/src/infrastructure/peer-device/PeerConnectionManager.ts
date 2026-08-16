/**
 * Peer connection lifecycle, outside React.
 *
 * Attaching to a peer is a long-lived state machine — handshake, capability
 * negotiation, keepalive, backoff, teardown — and it has to outlive whatever is
 * on screen: attachments survive surface switches, keep running our work, and
 * must not restart because a provider re-rendered. Expressed as effects it
 * became several timers and async closures racing over refs, where a switch
 * during a reconnect could leave a peer attached with no keepalive, or detached
 * with a timer still firing.
 *
 * So the lifecycle lives here as plain state with explicit transitions, and
 * React only subscribes: `subscribe()` + `list()` feed a component, `get()`
 * hands back the transport adapter, `dispose()` tears one down deterministically.
 * Nothing in this file may import React.
 */

import { PeerDeviceTransportAdapter } from '@/infrastructure/api/adapters/peer-device-adapter';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { createLogger } from '@/shared/utils/logger';
import type { DeviceSurfaceId } from './deviceSurface';

const log = createLogger('PeerConnectionManager');

export const PEER_CONTROL_RPC_TIMEOUT_MS = 15_000;
export const PEER_KEEPALIVE_INTERVAL_MS = 20_000;
/** Consecutive keepalive/reconnect failures tolerated before a peer is lost. */
export const PEER_MAX_KEEPALIVE_FAILURES = 3;
export const PEER_RECONNECT_BASE_DELAY_MS = 1_000;
export const PEER_RECONNECT_MAX_DELAY_MS = 15_000;

/**
 * `connecting` → first handshake. `ready` → the peer answers. `degraded` → it
 * missed at least one ping and we are retrying; the attachment is still valid
 * and its work still runs. `lost` → terminal, nothing is retried; the consumer
 * decides whether to dispose.
 */
export type PeerConnectionHealth = 'connecting' | 'ready' | 'degraded' | 'lost';

export type PeerConnectionLostReason = 'keepalive' | 'presence';

export interface PeerHostCapabilities {
  readonly idempotentDialogSubmit: boolean;
  readonly targetedSessionRollback: boolean;
}

/** Immutable view of one connection; safe to hold in component state. */
export interface PeerConnectionState {
  readonly deviceId: string;
  readonly deviceName: string;
  readonly surfaceId: DeviceSurfaceId;
  readonly health: PeerConnectionHealth;
  readonly capabilities: PeerHostCapabilities;
  /** Failures since the last answered ping; drives the backoff schedule. */
  readonly consecutiveFailures: number;
  readonly lostReason: PeerConnectionLostReason | null;
}

/** Live handle. `adapter` is the product transport for this device. */
export interface PeerConnection {
  readonly deviceId: string;
  readonly surfaceId: DeviceSurfaceId;
  readonly adapter: PeerDeviceTransportAdapter;
  getState(): PeerConnectionState;
}

export type PeerConnectionListener = (connections: readonly PeerConnectionState[]) => void;

export type PeerDeviceRpc = (
  targetDeviceId: string,
  commandJson: string,
  timeoutMs?: number,
) => Promise<string>;

export interface PeerConnectionManagerOptions {
  deviceRpc?: PeerDeviceRpc;
  getControllerDeviceId?: () => Promise<string>;
  controlRpcTimeoutMs?: number;
  keepaliveIntervalMs?: number;
  maxKeepaliveFailures?: number;
  reconnectBaseDelayMs?: number;
  reconnectMaxDelayMs?: number;
}

export interface PeerDisposeOptions {
  /**
   * Send `peer_control_detach`. Skip it for a peer that is already gone — the
   * host prunes the stale controller through presence instead.
   */
  notifyPeer?: boolean;
}

interface HostInvokeEnvelope {
  resp?: string;
  ok?: boolean;
  value?: unknown;
  error?: string;
  message?: string;
}

interface PeerModePingResult {
  capabilities?: {
    idempotent_dialog_submit?: boolean;
    targeted_session_rollback?: boolean;
  };
}

const NO_CAPABILITIES: PeerHostCapabilities = {
  idempotentDialogSubmit: false,
  targetedSessionRollback: false,
};

interface ConnectionEntry {
  deviceId: string;
  deviceName: string;
  adapter: PeerDeviceTransportAdapter;
  health: PeerConnectionHealth;
  capabilities: PeerHostCapabilities;
  consecutiveFailures: number;
  lostReason: PeerConnectionLostReason | null;
  timer: ReturnType<typeof setTimeout> | null;
  disposed: boolean;
  reattachInFlight: Promise<void> | null;
  handle: PeerConnection;
}

class PeerConnectionDisposedError extends Error {
  constructor(deviceId: string) {
    super(`Peer connection '${deviceId}' was disposed during attachment`);
    this.name = 'PeerConnectionDisposedError';
  }
}

export class PeerConnectionManager {
  private readonly entries = new Map<string, ConnectionEntry>();
  private readonly attaching = new Map<string, Promise<PeerConnection>>();
  private readonly listeners = new Set<PeerConnectionListener>();
  private readonly deviceRpc: PeerDeviceRpc;
  private readonly resolveControllerDeviceId: () => Promise<string>;
  private readonly controlRpcTimeoutMs: number;
  private readonly keepaliveIntervalMs: number;
  private readonly maxKeepaliveFailures: number;
  private readonly reconnectBaseDelayMs: number;
  private readonly reconnectMaxDelayMs: number;
  private controllerDeviceId: string | null = null;
  private snapshot: readonly PeerConnectionState[] = [];

  constructor(options: PeerConnectionManagerOptions = {}) {
    this.deviceRpc = options.deviceRpc
      ?? ((target, commandJson, timeoutMs) =>
        remoteConnectAPI.accountDeviceRpc(target, commandJson, timeoutMs));
    this.resolveControllerDeviceId = options.getControllerDeviceId
      ?? (async () => (await remoteConnectAPI.getDeviceInfo()).device_id);
    this.controlRpcTimeoutMs = options.controlRpcTimeoutMs ?? PEER_CONTROL_RPC_TIMEOUT_MS;
    this.keepaliveIntervalMs = options.keepaliveIntervalMs ?? PEER_KEEPALIVE_INTERVAL_MS;
    this.maxKeepaliveFailures = options.maxKeepaliveFailures ?? PEER_MAX_KEEPALIVE_FAILURES;
    this.reconnectBaseDelayMs = options.reconnectBaseDelayMs ?? PEER_RECONNECT_BASE_DELAY_MS;
    this.reconnectMaxDelayMs = options.reconnectMaxDelayMs ?? PEER_RECONNECT_MAX_DELAY_MS;
  }

  /**
   * Attach to `deviceId`, or return the existing connection. Attaching does not
   * render the device; the caller decides what the window draws.
   *
   * Rejects (leaving nothing behind) when the handshake fails, so a caller can
   * report the failure without first having to clean up a half-attached peer.
   */
  connect(deviceId: string, deviceName = ''): Promise<PeerConnection> {
    if (!deviceId) {
      return Promise.reject(new Error('deviceId is required'));
    }
    const existing = this.entries.get(deviceId);
    if (existing) {
      if (existing.health === 'lost') {
        return Promise.reject(new Error(`Peer device '${deviceId}' is no longer reachable`));
      }
      this.renameEntry(existing, deviceName);
      return Promise.resolve(existing.handle);
    }
    const inFlight = this.attaching.get(deviceId);
    if (inFlight) {
      return inFlight;
    }
    const attach = this.attach(deviceId, deviceName).finally(() => {
      this.attaching.delete(deviceId);
    });
    this.attaching.set(deviceId, attach);
    return attach;
  }

  get(deviceId: string): PeerConnection | undefined {
    return this.entries.get(deviceId)?.handle;
  }

  has(deviceId: string): boolean {
    return this.entries.has(deviceId);
  }

  /** Stable between changes, so a React consumer can compare by reference. */
  list(): readonly PeerConnectionState[] {
    return this.snapshot;
  }

  /** Listeners fire on change only; read `list()` for the current value. */
  subscribe(listener: PeerConnectionListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /**
   * End a control link for good: stop its timers, settle everything still
   * waiting on its transport, then tell the peer.
   *
   * The detach RPC is last and its failure is re-thrown, because a peer that
   * did not confirm may still be running our work — the caller decides whether
   * that is worth surfacing. Local teardown has already completed either way.
   */
  async dispose(deviceId: string, options: PeerDisposeOptions = {}): Promise<void> {
    const entry = this.entries.get(deviceId);
    if (!entry) {
      return;
    }
    entry.disposed = true;
    this.entries.delete(deviceId);
    this.cancelTimer(entry);
    this.publish();

    try {
      await entry.adapter.disconnect();
    } catch (error) {
      log.warn('Peer transport teardown failed', { deviceId, error });
    }

    // If disposal raced the handshake, let the attach task observe `disposed`
    // before the final detach. In particular, when `peer_control_attach` was
    // already in flight, detach must be ordered after it or the late attach
    // would resurrect a control link the controller considers closed.
    await this.attaching.get(deviceId)?.catch(() => undefined);
    await entry.reattachInFlight?.catch(() => undefined);

    log.info('Peer connection disposed', { deviceId });

    if (options.notifyPeer !== false) {
      await this.hostInvoke(deviceId, 'peer_control_detach', {
        controller_device_id: await this.controllerId(),
      });
    }
  }

  async disposeAll(options: PeerDisposeOptions = {}): Promise<void> {
    for (const deviceId of Array.from(this.entries.keys())) {
      try {
        await this.dispose(deviceId, options);
      } catch (error) {
        log.warn('Failed to dispose peer connection', { deviceId, error });
      }
    }
  }

  /**
   * Account presence is the authority on reachability: a peer that dropped off
   * cannot be running our work, so no amount of backoff will help it.
   */
  reportPresence(onlineDeviceIds: Iterable<string>): void {
    const online = new Set(onlineDeviceIds);
    for (const entry of this.entries.values()) {
      if (!online.has(entry.deviceId)) {
        this.markLost(entry, 'presence');
      }
    }
  }

  private async attach(deviceId: string, deviceName: string): Promise<PeerConnection> {
    const adapter = new PeerDeviceTransportAdapter(
      deviceId,
      (target, commandJson, timeoutMs) => this.deviceRpc(target, commandJson, timeoutMs),
      {
        // Successful product traffic proves the link as well as a ping does.
        onHostInvokeSuccess: () => this.noteHealthy(deviceId),
        onHostInvokeTransportFailure: error => {
          const current = this.entries.get(deviceId);
          if (current) {
            this.noteFailure(current, error);
          }
        },
      },
    );
    const entry: ConnectionEntry = {
      deviceId,
      deviceName,
      adapter,
      health: 'connecting',
      capabilities: NO_CAPABILITIES,
      consecutiveFailures: 0,
      lostReason: null,
      timer: null,
      disposed: false,
      reattachInFlight: null,
      handle: {
        deviceId,
        surfaceId: adapter.surfaceId,
        adapter,
        getState: () => this.stateOf(entry),
      },
    };
    this.entries.set(deviceId, entry);
    this.publish();

    try {
      await adapter.connect();
      this.assertEntryActive(entry);
      entry.capabilities = await this.probeCapabilities(deviceId);
      this.assertEntryActive(entry);
      await this.sendAttach(deviceId);
      this.assertEntryActive(entry);
    } catch (error) {
      if (this.entries.get(deviceId) === entry) {
        this.entries.delete(deviceId);
      }
      await adapter.disconnect().catch(() => undefined);
      this.publish();
      throw error;
    }

    adapter.setHostCapabilities({
      supportsIdempotentDialogSubmit: entry.capabilities.idempotentDialogSubmit,
      supportsTargetedSessionRollback: entry.capabilities.targetedSessionRollback,
    });
    entry.health = 'ready';
    this.scheduleKeepalive(entry);
    this.publish();
    log.info('Peer connection ready', { deviceId, capabilities: entry.capabilities });
    return entry.handle;
  }

  private async probeCapabilities(deviceId: string): Promise<PeerHostCapabilities> {
    const result = await this.hostInvoke<PeerModePingResult>(deviceId, 'peer_mode_ping', {});
    return {
      idempotentDialogSubmit: result?.capabilities?.idempotent_dialog_submit === true,
      targetedSessionRollback: result?.capabilities?.targeted_session_rollback === true,
    };
  }

  private async sendAttach(deviceId: string): Promise<void> {
    await this.hostInvoke(deviceId, 'peer_control_attach', {
      controller_device_id: await this.controllerId(),
    });
  }

  private scheduleKeepalive(entry: ConnectionEntry): void {
    this.cancelTimer(entry);
    entry.timer = setTimeout(() => {
      entry.timer = null;
      void this.runHealthCheck(entry);
    }, this.keepaliveIntervalMs);
  }

  /**
   * Backoff is driven by the failure count, so a peer that keeps missing pings
   * is polled progressively less often instead of hammering a weak relay.
   */
  private scheduleReconnect(entry: ConnectionEntry): void {
    this.cancelTimer(entry);
    const delayMs = Math.min(
      this.reconnectBaseDelayMs * (2 ** Math.max(0, entry.consecutiveFailures - 1)),
      this.reconnectMaxDelayMs,
    );
    entry.timer = setTimeout(() => {
      entry.timer = null;
      void this.runHealthCheck(entry);
    }, delayMs);
  }

  /**
   * One probe covers both roles: while `ready` it is the keepalive, while
   * `degraded` it is the reconnect attempt. A degraded peer re-sends
   * `peer_control_attach` because the host may have pruned this controller
   * during the gap that made us degraded in the first place.
   */
  private async runHealthCheck(entry: ConnectionEntry): Promise<void> {
    if (this.entries.get(entry.deviceId) !== entry || entry.health === 'lost') {
      return;
    }
    const reconnecting = entry.health === 'degraded';
    try {
      const capabilities = await this.probeCapabilities(entry.deviceId);
      if (reconnecting) {
        await this.sendAttach(entry.deviceId);
      }
      if (this.entries.get(entry.deviceId) !== entry) {
        return;
      }
      entry.capabilities = capabilities;
      entry.adapter.setHostCapabilities({
        supportsIdempotentDialogSubmit: capabilities.idempotentDialogSubmit,
        supportsTargetedSessionRollback: capabilities.targetedSessionRollback,
      });
      entry.consecutiveFailures = 0;
      const recovered = entry.health !== 'ready';
      entry.health = 'ready';
      this.scheduleKeepalive(entry);
      if (recovered) {
        log.info('Peer connection recovered', { deviceId: entry.deviceId });
        this.publish();
      }
    } catch (error) {
      this.noteFailure(entry, error);
    }
  }

  /**
   * A single miss on a weak link must not drop a peer that is mid-turn, so the
   * first failure only degrades the connection. Only repeated failures mean the
   * peer is really unreachable.
   */
  private noteFailure(entry: ConnectionEntry, error: unknown): void {
    if (this.entries.get(entry.deviceId) !== entry || entry.health === 'lost') {
      return;
    }
    entry.consecutiveFailures += 1;
    log.warn('Peer keepalive failed', {
      deviceId: entry.deviceId,
      consecutiveFailures: entry.consecutiveFailures,
      error,
    });
    if (entry.consecutiveFailures >= this.maxKeepaliveFailures) {
      this.markLost(entry, 'keepalive');
      return;
    }
    entry.health = 'degraded';
    this.scheduleReconnect(entry);
    this.publish();
  }

  private noteHealthy(deviceId: string): void {
    const entry = this.entries.get(deviceId);
    // A lost connection is terminal: it is disposed, not silently revived.
    if (!entry || entry.health !== 'degraded') {
      return;
    }
    entry.consecutiveFailures = 0;
    entry.health = 'ready';
    this.scheduleKeepalive(entry);
    this.publish();

    // HostInvoke itself does not require an attachment, so successful product
    // traffic proves reachability but not that DeviceEvents are still fanned
    // out. Re-attach in the background before treating recovery as durable.
    if (!entry.reattachInFlight) {
      const reattach = this.sendAttach(deviceId)
        .catch(error => this.noteFailure(entry, error))
        .finally(() => {
          if (entry.reattachInFlight === reattach) {
            entry.reattachInFlight = null;
          }
        });
      entry.reattachInFlight = reattach;
    }
  }

  private markLost(entry: ConnectionEntry, reason: PeerConnectionLostReason): void {
    if (entry.health === 'lost') {
      return;
    }
    this.cancelTimer(entry);
    entry.health = 'lost';
    entry.lostReason = reason;
    log.warn('Peer connection lost', { deviceId: entry.deviceId, reason });
    this.publish();
  }

  private cancelTimer(entry: ConnectionEntry): void {
    if (entry.timer !== null) {
      clearTimeout(entry.timer);
      entry.timer = null;
    }
  }

  private assertEntryActive(entry: ConnectionEntry): void {
    if (entry.disposed || this.entries.get(entry.deviceId) !== entry) {
      throw new PeerConnectionDisposedError(entry.deviceId);
    }
  }

  private renameEntry(entry: ConnectionEntry, deviceName: string): void {
    if (!deviceName || entry.deviceName === deviceName) {
      return;
    }
    entry.deviceName = deviceName;
    this.publish();
  }

  private async controllerId(): Promise<string> {
    if (this.controllerDeviceId === null) {
      this.controllerDeviceId = await this.resolveControllerDeviceId();
    }
    return this.controllerDeviceId;
  }

  private async hostInvoke<T>(
    deviceId: string,
    command: string,
    args: Record<string, unknown>,
  ): Promise<T | undefined> {
    const raw = await this.deviceRpc(
      deviceId,
      JSON.stringify({ cmd: 'host_invoke', command, args }),
      this.controlRpcTimeoutMs,
    );
    const envelope = JSON.parse(raw) as HostInvokeEnvelope;
    if (envelope.resp === 'error') {
      throw new Error(envelope.message || `Peer '${command}' failed`);
    }
    if (envelope.resp === 'host_invoke_result' && !envelope.ok) {
      throw new Error(envelope.error || `Peer '${command}' failed`);
    }
    return envelope.value as T | undefined;
  }

  private stateOf(entry: ConnectionEntry): PeerConnectionState {
    return {
      deviceId: entry.deviceId,
      deviceName: entry.deviceName,
      surfaceId: entry.handle.surfaceId,
      health: entry.health,
      capabilities: entry.capabilities,
      consecutiveFailures: entry.consecutiveFailures,
      lostReason: entry.lostReason,
    };
  }

  private publish(): void {
    this.snapshot = Array.from(this.entries.values(), entry => this.stateOf(entry));
    for (const listener of Array.from(this.listeners)) {
      try {
        listener(this.snapshot);
      } catch (error) {
        log.warn('Peer connection listener threw', error);
      }
    }
  }
}

/** Window-wide instance; peer links outlive any component that renders them. */
export const peerConnectionManager = new PeerConnectionManager();
