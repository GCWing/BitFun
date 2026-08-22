import { RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';

export type DeviceDirectoryStatus = 'idle' | 'cached' | 'loading' | 'ready' | 'failed';

/**
 * One account desktop in the sidebar. Nested @Trace fields are what
 * SidebarDeviceGroup actually watches — replacing a plain object on
 * the parent array does not update a reused ForEach child.
 */
@ObservedV2
export class DeviceDirectoryEntry {
  @Trace deviceId: string = '';
  @Trace deviceName: string = '';
  @Trace online: boolean = false;
  @Trace expanded: boolean = false;
  @Trace status: DeviceDirectoryStatus = 'idle';
  @Trace errorText: string = '';
  @Trace workspaces: RecentWorkspaceEntry[] = [];
  @Trace sessions: RemoteSession[] = [];
}

/** Snapshot of every account desktop the sidebar can observe. */
@ObservedV2
export class DeviceDirectoryState {
  @Trace devices: DeviceDirectoryEntry[] = [];

  replace(devices: DeviceDirectoryEntry[]): void {
    this.devices = devices;
  }

  clear(): void {
    this.devices = [];
  }

  find(deviceId: string): DeviceDirectoryEntry | undefined {
    return this.devices.find((item: DeviceDirectoryEntry): boolean => item.deviceId === deviceId);
  }
}

export function emptyDeviceDirectoryEntry(
  deviceId: string,
  deviceName: string,
  online: boolean
): DeviceDirectoryEntry {
  const entry = new DeviceDirectoryEntry();
  entry.deviceId = deviceId;
  entry.deviceName = deviceName;
  entry.online = online;
  return entry;
}
