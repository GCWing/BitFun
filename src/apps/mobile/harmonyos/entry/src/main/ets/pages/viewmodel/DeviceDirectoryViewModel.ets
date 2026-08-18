import { RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';
import {
  AccountDeviceDirectory,
  AccountDeviceDirectorySource
} from '../../services/AccountDeviceDirectory';
import { CloudAccountBinding, CloudAccountDevice } from '../../services/CloudAccountClient';
import { DeviceDirectoryCoordinator } from '../../services/DeviceDirectoryCoordinator';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteLogger } from '../../services/RemoteLogger';
import { RemoteSessionListStore } from '../../services/RemoteSessionListCache';
import { stampRemoteSessionsDevice } from '../../services/RemoteSessionIdentity';
import {
  DeviceDirectoryEntry,
  DeviceDirectoryState,
  emptyDeviceDirectoryEntry
} from '../state/DeviceDirectoryState';

export interface DeviceDirectoryViewModelHooks {
  readonly credentials: () => CloudAccountBinding | undefined;
  readonly activeDeviceId: () => string;
  /** Whether the conversation link to [activeDeviceId] is currently live. */
  readonly activeDeviceConnected: () => boolean;
}

/** Expand / collapse / retry for the sidebar's per-device directory. */
export class DeviceDirectoryViewModel {
  private readonly state: DeviceDirectoryState;
  private readonly store?: RemoteSessionListStore;
  private readonly hooks: DeviceDirectoryViewModelHooks;
  private readonly directory?: AccountDeviceDirectorySource;
  private readonly inFlight: Set<string> = new Set<string>();

  constructor(
    state: DeviceDirectoryState,
    hooks: DeviceDirectoryViewModelHooks,
    store?: RemoteSessionListStore,
    directory?: AccountDeviceDirectorySource
  ) {
    this.state = state;
    this.hooks = hooks;
    this.store = store;
    this.directory = directory;
  }

  /**
   * Re-runs the device sync against the current active id.
   *
   * Nothing about a row's own fields changes here — that is the point. It is
   * the seam the control-target switch goes through, so what it guards is that
   * a catalog captured off the live page survives the device ceasing to be
   * live.
   */
  refreshActive(): void {
    this.syncDevices(this.state.devices.map((item: DeviceDirectoryEntry): CloudAccountDevice => {
      return {
        deviceId: item.deviceId,
        deviceName: item.deviceName,
        online: item.online
      };
    }));
  }

  /**
   * Mirrors the live conversation device onto its directory row.
   *
   * The active row reads RemotePageState, so this snapshot is what remains
   * after the control target moves on. Empty payloads are ignored so the
   * teardown that clears the live page cannot wipe a row we just captured.
   */
  captureLive(
    deviceId: string,
    workspaces: RecentWorkspaceEntry[],
    sessions: RemoteSession[]
  ): void {
    const target = deviceId.trim();
    if (target.length === 0) {
      return;
    }
    const entry = this.state.find(target);
    if (!entry) {
      return;
    }
    if (workspaces.length === 0 && sessions.length === 0) {
      return;
    }
    if (workspaces.length > 0) {
      entry.workspaces = workspaces;
    }
    if (sessions.length > 0) {
      entry.sessions = stampRemoteSessionsDevice(sessions, target);
    }
    if (entry.status === 'idle' || entry.status === 'cached') {
      entry.status = 'ready';
    }
  }

  syncDevices(devices: CloudAccountDevice[]): void {
    const activeId = this.hooks.activeDeviceId();
    const activeConnected = this.hooks.activeDeviceConnected();
    const next: DeviceDirectoryEntry[] = devices.map((device: CloudAccountDevice): DeviceDirectoryEntry => {
      // The relay's presence flag is hearsay: it is a projection of a socket
      // registry that can lag or lose an entry. A live conversation link is
      // first-hand proof, so it wins. Without this the desktop we are pinging
      // every fifteen seconds can render as a greyed-out offline row.
      const online = device.online ||
        (device.deviceId === activeId && activeId.length > 0 && activeConnected);
      if (online !== device.online) {
        // Only the disagreement is worth a line. It is rare, it is the whole
        // reason this branch exists, and it is what the next person chasing a
        // wrongly-greyed row will grep for.
        RemoteLogger.info(`device presence override device=${device.deviceId} relay=offline link=live`);
      }
      const existing = this.state.find(device.deviceId);
      if (existing) {
        existing.deviceName = device.deviceName || existing.deviceName;
        if (existing.online && !online) {
          existing.expanded = false;
        }
        existing.online = online;
        return existing;
      }
      const isActive = device.deviceId === activeId && online;
      const created = emptyDeviceDirectoryEntry(
        device.deviceId,
        device.deviceName || device.deviceId,
        online
      );
      created.expanded = isActive;
      return created;
    });
    this.state.replace(next);
  }

  async hydrateKnownDevices(): Promise<void> {
    const devices = this.state.devices;
    for (let index = 0; index < devices.length; index++) {
      await this.hydrateDevice(devices[index]);
    }
  }

  async toggleDevice(deviceId: string): Promise<void> {
    const entry = this.state.find(deviceId);
    if (!entry) {
      return;
    }
    entry.expanded = !entry.expanded;
    RemoteLogger.info(`device directory toggle device=${deviceId} expanded=${entry.expanded}`);
    if (!entry.expanded) {
      return;
    }
    if (this.shouldFetch(entry)) {
      entry.status = 'loading';
      entry.errorText = '';
    }
    await this.ensureLoaded(deviceId);
  }

  async retryDevice(deviceId: string): Promise<void> {
    const entry = this.state.find(deviceId);
    if (!entry) {
      return;
    }
    entry.expanded = true;
    await this.loadDevice(deviceId);
  }

  async ensureLoaded(deviceId: string): Promise<void> {
    const entry = this.state.find(deviceId);
    if (!entry || !entry.expanded) {
      return;
    }
    if (deviceId === this.hooks.activeDeviceId()) {
      return;
    }
    if (!entry.online) {
      return;
    }
    if (entry.status === 'ready') {
      return;
    }
    await this.loadDevice(deviceId);
  }

  private shouldFetch(entry: DeviceDirectoryEntry): boolean {
    if (entry.deviceId === this.hooks.activeDeviceId()) {
      return false;
    }
    if (!entry.online) {
      return false;
    }
    return entry.status === 'idle' || entry.status === 'cached' || entry.status === 'failed';
  }

  private async loadDevice(deviceId: string): Promise<void> {
    const entry = this.state.find(deviceId);
    if (!entry) {
      return;
    }
    if (deviceId === this.hooks.activeDeviceId()) {
      return;
    }
    if (!entry.online) {
      return;
    }
    if (this.inFlight.has(deviceId)) {
      return;
    }
    const source = this.directorySource();
    if (!source) {
      entry.status = 'failed';
      entry.errorText = RemoteI18n.t('remote.settings.accountNotSignedIn');
      return;
    }
    this.inFlight.add(deviceId);
    entry.status = 'loading';
    entry.errorText = '';
    try {
      const coordinator = new DeviceDirectoryCoordinator(source);
      entry.workspaces = await coordinator.loadWorkspaces(deviceId);
      entry.sessions = await coordinator.loadSessions(deviceId, entry.workspaces);
      entry.status = 'ready';
      entry.errorText = '';
      await this.persistDevice(deviceId, entry.workspaces, entry.sessions);
    } catch (err) {
      // The retry row carries a localized line of its own. What the transport
      // throws is English and frequently a stack, so it stays in the log, which
      // is the only place it reads as anything but noise.
      RemoteLogger.warn(`device directory load failed device=${deviceId}: ${String(err)}`);
      entry.status = 'failed';
      entry.errorText = '';
    } finally {
      this.inFlight.delete(deviceId);
    }
  }

  private async hydrateDevice(entry: DeviceDirectoryEntry): Promise<void> {
    const workspaces = this.store ?
      await DeviceDirectoryViewModel.safeWorkspaces(this.store, entry.deviceId) : [];
    const sessions = this.store ?
      await DeviceDirectoryViewModel.safeSessions(this.store, entry.deviceId) : [];
    if (workspaces.length === 0 && sessions.length === 0) {
      return;
    }
    if (workspaces.length > 0) {
      entry.workspaces = workspaces;
    }
    if (sessions.length > 0) {
      entry.sessions = stampRemoteSessionsDevice(sessions, entry.deviceId);
    }
    if (entry.status === 'idle') {
      entry.status = 'cached';
    }
  }

  private directorySource(): AccountDeviceDirectorySource | undefined {
    if (this.directory) {
      return this.directory;
    }
    const context = this.hooks.credentials();
    return context ? new AccountDeviceDirectory(context) : undefined;
  }

  private async persistDevice(
    deviceId: string,
    workspaces: RecentWorkspaceEntry[],
    sessions: RemoteSession[]
  ): Promise<void> {
    try {
      if (this.store) {
        await this.store.saveWorkspaces(deviceId, workspaces);
        await this.store.saveObservedList(deviceId, sessions, false);
      }
    } catch (err) {
      RemoteLogger.warn(`device directory persist failed device=${deviceId}: ${String(err)}`);
    }
  }

  private static async safeWorkspaces(
    store: RemoteSessionListStore,
    deviceId: string
  ): Promise<RecentWorkspaceEntry[]> {
    try {
      return await store.loadWorkspaces(deviceId);
    } catch (err) {
      RemoteLogger.warn(`device workspace cache read failed device=${deviceId}: ${String(err)}`);
      return [];
    }
  }

  private static async safeSessions(
    store: RemoteSessionListStore,
    deviceId: string
  ): Promise<RemoteSession[]> {
    try {
      return (await store.loadList(deviceId)).sessions;
    } catch (err) {
      RemoteLogger.warn(`device session cache read failed device=${deviceId}: ${String(err)}`);
      return [];
    }
  }
}
