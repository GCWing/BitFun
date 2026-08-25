import { RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import {
  DeviceDirectoryEntry,
  DeviceDirectoryState,
  emptyDeviceDirectoryEntry
} from '../state/DeviceDirectoryState';
import { MUTED } from './Theme';
import { SidebarDirectoryPreviewPolicy } from '../policy/SidebarDirectoryPreviewPolicy';
import { SidebarDeviceGroup } from './SidebarDeviceGroup';

/**
 * The workspace half of the unified sidebar.
 *
 * Account desktops always render a device header so the row keeps a real
 * name. A QR pairing with no account device list still flattens.
 */
@ComponentV2
export struct SidebarWorkspaceSection {
  @Param directoryState: DeviceDirectoryState = new DeviceDirectoryState();
  @Param sessions: RemoteSession[] = [];
  @Param recentWorkspaces: RecentWorkspaceEntry[] = [];
  @Param workspaceName: string = '';
  @Param workspacePath: string = '';
  @Param workspaceKind: string = 'normal';
  @Param connectionState: string = 'idle';
  @Param controlTargetType: string = 'none';
  @Param controlTargetDeviceId: string = '';
  @Param controlTargetDeviceName: string = '';
  @Param selectedSessionId: string = '';
  @Param runningSessionId: string = '';
  @Param query: string = '';
  @Event onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onSessionActions: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onCreateInWorkspace: (deviceId: string, path: string) => void =
    (_deviceId: string, _path: string) => {};
  @Event onOpenWorkspace: (deviceId: string, path: string) => void =
    (_deviceId: string, _path: string) => {};
  @Event onToggleDevice: (deviceId: string) => void = (_deviceId: string) => {};
  @Event onRetryDevice: (deviceId: string) => void = (_deviceId: string) => {};
  @Event onAdd: () => void = () => {};
  @Local visibleDeviceCount: number = SidebarDirectoryPreviewPolicy.PREVIEW_COUNT;
  private readonly unpairedEntry: DeviceDirectoryEntry = emptyDeviceDirectoryEntry('', '', false);

  build() {
    Column() {
      this.SectionHeader()
      if (this.deviceEntries().length > 0) {
        ForEach(this.visibleDeviceEntries(), (entry: DeviceDirectoryEntry) => {
          SidebarDeviceGroup({
            device: entry,
            sessions: this.sessionsForDevice(entry),
            recentWorkspaces: this.workspacesForDevice(entry),
            workspaceName: this.liveWorkspaceName(entry),
            workspacePath: this.liveWorkspacePath(entry),
            workspaceKind: this.liveWorkspaceKind(entry),
            selectedSessionId: this.selectedSessionIdFor(entry),
            runningSessionId: this.isActiveDevice(entry.deviceId) ? this.runningSessionId : '',
            query: this.query,
            showHeader: this.showsDeviceHeader(),
            isActive: this.isActiveDevice(entry.deviceId),
            onToggle: () => {
              this.onToggleDevice(entry.deviceId);
            },
            onRetry: () => {
              this.onRetryDevice(entry.deviceId);
            },
            onOpenSession: this.onOpenSession,
            onSessionActions: this.onSessionActions,
            onCreateInWorkspace: (path: string) => {
              this.onCreateInWorkspace(entry.deviceId, path);
            },
            onOpenWorkspace: (path: string) => {
              this.onOpenWorkspace(entry.deviceId, path);
            }
          })
        }, (entry: DeviceDirectoryEntry): string => this.deviceKey(entry))
        if (this.visibleDeviceCount < this.deviceEntries().length) {
          this.MoreDevicesRow()
        }
      }
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
    .margin({ top: 18 })
  }

  @Builder
  private SectionHeader() {
    Row() {
      Text(RemoteI18n.t('sidebar.devices'))
        .id('sidebar-workspace-section')
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
      Blank()
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.plus'))
          .fontSize(17)
          .fontColor([MUTED])
      }
      .width(32)
      .height(32)
      .accessibilityText(RemoteI18n.t('sidebar.addConnection'))
      .onClick(() => {
        this.onAdd();
      })
    }
    .width('100%')
    .height(38)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private MoreDevicesRow() {
    Row({ space: 8 }) {
      Text('···')
        .fontSize(13)
        .fontColor(MUTED)
      Text(RemoteI18n.f('sidebar.moreDevices',
        String(this.deviceEntries().length - this.visibleDeviceCount)))
        .fontSize(13)
        .fontColor(MUTED)
    }
    .width('100%')
    .height(40)
    .padding({ left: 10 })
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      this.visibleDeviceCount += SidebarDirectoryPreviewPolicy.PREVIEW_COUNT;
    })
  }

  private deviceEntries(): DeviceDirectoryEntry[] {
    if (this.directoryState.devices.length > 0) {
      return this.directoryState.devices;
    }
    if (!this.isConnected() && this.controlTargetType === 'none') {
      return [];
    }
    if (!this.isConnected() && this.recentWorkspaces.length === 0 && this.sessions.length === 0) {
      return [];
    }
    this.unpairedEntry.deviceId = this.controlTargetDeviceId;
    this.unpairedEntry.deviceName = this.controlTargetDeviceName;
    this.unpairedEntry.online = this.isConnected();
    this.unpairedEntry.expanded = this.isConnected();
    this.unpairedEntry.status = 'ready';
    this.unpairedEntry.workspaces = this.recentWorkspaces;
    this.unpairedEntry.sessions = this.sessions;
    return [this.unpairedEntry];
  }

  private visibleDeviceEntries(): DeviceDirectoryEntry[] {
    const entries = this.deviceEntries();
    if (entries.length <= 1) {
      return entries;
    }
    return entries.length <= this.visibleDeviceCount ?
      entries : entries.slice(0, this.visibleDeviceCount);
  }

  /**
   * ForEach binds a child to the item it was created with and only re-reads
   * that item when the key changes. The unpaired placeholder carries the
   * control target's own deviceId, so an unprefixed key collides with the real
   * row for that same desktop: the child built during the flatten phase keeps
   * rendering the frozen placeholder after the account device list lands —
   * greyed out and unresponsive to its own toggle, because every field it
   * paints belongs to an object nothing updates any more.
   */
  private deviceKey(entry: DeviceDirectoryEntry): string {
    return this.directoryState.devices.length > 0 ? entry.deviceId : `unpaired:${entry.deviceId}`;
  }

  private showsDeviceHeader(): boolean {
    return this.directoryState.devices.length > 0;
  }

  private usesLiveDirectory(entry: DeviceDirectoryEntry): boolean {
    return this.isActiveDevice(entry.deviceId) && this.isConnected();
  }

  private liveWorkspacePath(entry: DeviceDirectoryEntry): string {
    return this.usesLiveDirectory(entry) ? this.workspacePath : '';
  }

  private liveWorkspaceName(entry: DeviceDirectoryEntry): string {
    return this.liveWorkspacePath(entry).length > 0 ? this.workspaceName : '';
  }

  private liveWorkspaceKind(entry: DeviceDirectoryEntry): string {
    return this.usesLiveDirectory(entry) ? this.workspaceKind : 'normal';
  }

  private sessionsForDevice(entry: DeviceDirectoryEntry): RemoteSession[] {
    return this.usesLiveDirectory(entry) ? this.sessions : entry.sessions;
  }

  private workspacesForDevice(entry: DeviceDirectoryEntry): RecentWorkspaceEntry[] {
    return this.usesLiveDirectory(entry) ? this.recentWorkspaces : entry.workspaces;
  }

  private selectedSessionIdFor(entry: DeviceDirectoryEntry): string {
    if (this.selectedSessionId.length === 0) {
      return '';
    }
    if (this.isActiveDevice(entry.deviceId)) {
      return this.selectedSessionId;
    }
    if (entry.sessions.some((item: RemoteSession): boolean => item.id === this.selectedSessionId)) {
      return this.selectedSessionId;
    }
    return '';
  }

  private isActiveDevice(deviceId: string): boolean {
    const activeId = this.controlTargetDeviceId;
    if (activeId.length === 0) {
      return this.directoryState.devices.length === 0;
    }
    return deviceId === activeId;
  }

  private isConnected(): boolean {
    return this.connectionState === 'connected' || this.connectionState === 'reconnecting';
  }
}
