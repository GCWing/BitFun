import { RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import {
  DeviceDirectoryEntry,
  DeviceDirectoryState,
  emptyDeviceDirectoryEntry
} from '../state/DeviceDirectoryState';
import { INK, MUTED, SUBTLE } from './Theme';
import { SidebarDeviceGroup } from './SidebarDeviceGroup';

const DEVICE_BATCH: number = 3;

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
  @Event onConnect: () => void = () => {};
  @Local visibleDeviceCount: number = DEVICE_BATCH;
  private readonly unpairedEntry: DeviceDirectoryEntry = emptyDeviceDirectoryEntry('', '', false);

  build() {
    Column() {
      this.SectionHeader()
      if (this.deviceEntries().length === 0 && !this.isConnected()) {
        this.ConnectRow()
      } else if (this.deviceEntries().length === 0) {
        Text(RemoteI18n.t('sidebar.emptyWorkspaces'))
          .fontSize(14)
          .fontColor(MUTED)
          .width('100%')
          .padding({ left: 10, top: 6, bottom: 6 })
      } else {
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
        }, (entry: DeviceDirectoryEntry): string => entry.deviceId)
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
      if (!this.isConnected() && this.deviceEntries().length === 0) {
        Text(RemoteI18n.t('sidebar.workspacesOffline'))
          .fontSize(12)
          .fontColor(SUBTLE)
          .margin({ right: 8 })
      }
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.plus'))
          .fontSize(17)
          .fontColor([MUTED])
      }
      .width(32)
      .height(32)
      .accessibilityText(RemoteI18n.t('sidebar.addConnection'))
      .onClick(() => {
        this.onConnect();
      })
    }
    .width('100%')
    .height(38)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private ConnectRow() {
    Row({ space: 10 }) {
      SymbolGlyph($r('sys.symbol.desktop'))
        .fontSize(20)
        .fontColor([MUTED])
        .width(22)
        .height(22)
      Text(RemoteI18n.t('sidebar.connectDesktop'))
        .fontSize(15)
        .fontColor(INK)
        .layoutWeight(1)
      SymbolGlyph($r('sys.symbol.chevron_right'))
        .fontSize(13)
        .fontColor([MUTED])
    }
    .width('100%')
    .height(46)
    .padding({ left: 10, right: 8 })
    .borderRadius(10)
    .onClick(() => {
      this.onConnect();
    })
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
      this.visibleDeviceCount += DEVICE_BATCH;
    })
  }

  private deviceEntries(): DeviceDirectoryEntry[] {
    if (this.directoryState.devices.length > 0) {
      return this.directoryState.devices;
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
