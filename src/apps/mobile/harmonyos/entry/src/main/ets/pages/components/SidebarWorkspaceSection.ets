import { RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import {
  DeviceDirectoryEntry,
  DeviceDirectoryState,
  emptyDeviceDirectoryEntry
} from '../state/DeviceDirectoryState';
import { SidebarDirectoryPreviewPolicy } from '../policy/SidebarDirectoryPreviewPolicy';
import { SidebarDeviceProjectionPolicy } from '../policy/SidebarDeviceProjectionPolicy';
import {
  SettingsPlacement,
  SettingsPlacementPolicy,
  SettingsSheetKind
} from '../policy/SettingsPlacementPolicy';
import { AdaptiveSheetOptions } from './AdaptiveSheetOptions';
import { GREEN, INK, MUTED, PAGE_BG, SOFT, SUBTLE } from './Theme';
import { SidebarDeviceGroup } from './SidebarDeviceGroup';
import { SidebarWorkspacePicker } from './SidebarWorkspacePicker';

/**
 * Flat device selector followed by the selected device's workspace tree.
 *
 * Device rows choose the remote context; they no longer each own a nested
 * directory. This keeps one visible workspace hierarchy and makes its owner
 * explicit without repeating the same tree under every account desktop.
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
  @Param workspacePickerPlacement: SettingsPlacement =
    SettingsPlacementPolicy.compactBottom(SettingsSheetKind.SessionDetails);
  @Event onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onSessionActions: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onCreateInWorkspace: (deviceId: string, path: string) => void =
    (_deviceId: string, _path: string) => {};
  @Event onOpenWorkspace: (deviceId: string, path: string) => void =
    (_deviceId: string, _path: string) => {};
  @Event onSelectDevice: (deviceId: string) => void = (_deviceId: string) => {};
  @Event onRetryDevice: (deviceId: string) => void = (_deviceId: string) => {};
  @Event onAddWorkspace: (deviceId: string) => void = (_deviceId: string) => {};
  @Local visibleDeviceCount: number = SidebarDirectoryPreviewPolicy.PREVIEW_COUNT;
  @Local showWorkspacePicker: boolean = false;
  @Local workspacePickerDeviceId: string = '';
  private readonly transientEntry: DeviceDirectoryEntry = emptyDeviceDirectoryEntry('', '', false);

  build() {
    Column() {
      this.DeviceSectionHeader()
      if (this.deviceEntries().length > 0) {
        ForEach(this.visibleDeviceEntries(), (entry: DeviceDirectoryEntry) => {
          this.DeviceRow(entry)
        }, (entry: DeviceDirectoryEntry): string => this.deviceKey(entry))
        if (this.visibleDeviceCount < this.deviceEntries().length) {
          this.MoreDevicesRow()
        }
        if (this.selectedEntry()) {
          this.WorkspaceSection(this.selectedEntry() as DeviceDirectoryEntry)
        }
      }
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
    .margin({ top: 18 })
    .bindSheet($$this.showWorkspacePicker, this.WorkspacePicker(), this.workspacePickerSheetOptions())
  }

  @Builder
  private DeviceSectionHeader() {
    Text(RemoteI18n.t('sidebar.devices'))
      .id('sidebar-workspace-section')
      .fontSize(14)
      .fontWeight(FontWeight.Medium)
      .fontColor(MUTED)
      .width('100%')
      .height(38)
      .textAlign(TextAlign.Start)
  }

  @Builder
  private DeviceRow(entry: DeviceDirectoryEntry) {
    Row({ space: 12 }) {
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.desktop'))
          .fontSize(21)
          .fontColor([entry.online ? INK : SUBTLE])
          .hitTestBehavior(HitTestMode.None)
      }
      .width(28)
      .height(32)
      .hitTestBehavior(HitTestMode.None)
      Column({ space: 2 }) {
        Text(entry.deviceName)
          .fontSize(15)
          .fontWeight(this.isSelected(entry) ? FontWeight.Medium : FontWeight.Regular)
          .fontColor(entry.online ? INK : SUBTLE)
          .width('100%')
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .hitTestBehavior(HitTestMode.None)
        Row({ space: 6 }) {
          if (entry.online) {
            Text('')
              .width(7)
              .height(7)
              .backgroundColor(GREEN)
              .borderRadius(4)
              .hitTestBehavior(HitTestMode.None)
          }
          Text(RemoteI18n.t(entry.online ? 'sidebar.deviceOnline' : 'sidebar.deviceOffline'))
            .fontSize(12)
            .fontColor(entry.online ? GREEN : SUBTLE)
            .hitTestBehavior(HitTestMode.None)
        }
        .width('100%')
        .alignItems(VerticalAlign.Center)
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      if (entry.status === 'loading') {
        LoadingProgress()
          .width(14)
          .height(14)
          .color(MUTED)
          .hitTestBehavior(HitTestMode.None)
      }
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.chevron_right'))
          .fontSize(13)
          .fontColor([MUTED])
          .hitTestBehavior(HitTestMode.None)
      }
      .width(16)
      .height(20)
      .hitTestBehavior(HitTestMode.None)
    }
    .width('100%')
    .height(66)
    .padding({ left: 10, right: 8 })
    .alignItems(VerticalAlign.Center)
    .backgroundColor(this.isSelected(entry) ? SOFT : PAGE_BG)
    .borderRadius(16)
    .opacity(entry.online ? 1 : 0.72)
    .onClick(() => {
      this.onSelectDevice(entry.deviceId);
    })
  }

  @Builder
  private WorkspaceSection(entry: DeviceDirectoryEntry) {
    Column() {
      Row() {
        Text(RemoteI18n.t('sidebar.workspaces'))
          .fontSize(14)
          .fontWeight(FontWeight.Medium)
          .fontColor(MUTED)
        Blank()
        Stack({ alignContent: Alignment.Center }) {
          SymbolGlyph($r('sys.symbol.plus'))
            .fontSize(18)
            .fontColor([this.canUseDevice(entry) ? INK : SUBTLE])
        }
        .width(34)
        .height(34)
        .opacity(this.canUseDevice(entry) ? 1 : 0.38)
        .accessibilityText(RemoteI18n.t('sidebar.addWorkspace'))
        .onClick(() => {
          if (this.canUseDevice(entry)) {
            this.openWorkspacePicker(entry);
          }
        })
      }
      .width('100%')
      .height(48)
      .margin({ top: 16 })
      .alignItems(VerticalAlign.Center)

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
        showHeader: false,
        isActive: this.isActiveDevice(entry.deviceId),
        onRetry: () => {
          if (this.isDirectoryEntry(entry)) {
            this.onRetryDevice(entry.deviceId);
          }
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
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
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
    const directoryEntries = this.directoryState.devices;
    if (!SidebarDeviceProjectionPolicy.shouldPrependControlTarget(
      directoryEntries.map((entry: DeviceDirectoryEntry): string => entry.deviceId),
      this.controlTargetType,
      this.controlTargetDeviceId,
      this.isConnected(),
      this.recentWorkspaces.length > 0 || this.sessions.length > 0
    )) {
      return directoryEntries;
    }
    this.updateTransientEntry();
    return [this.transientEntry].concat(directoryEntries);
  }

  @Builder
  private WorkspacePicker() {
    SidebarWorkspacePicker({
      deviceName: this.workspacePickerDevice().deviceName,
      workspaces: this.workspacesForDevice(this.workspacePickerDevice()),
      selectedWorkspacePath: this.isActiveDevice(this.workspacePickerDevice().deviceId) ?
        this.workspacePath : '',
      loading: this.workspacePickerDevice().status === 'loading',
      onClose: () => {
        this.showWorkspacePicker = false;
      },
      onSelectWorkspace: (path: string) => {
        const deviceId = this.workspacePickerDeviceId;
        this.showWorkspacePicker = false;
        this.onOpenWorkspace(deviceId, path);
      }
    })
  }

  private openWorkspacePicker(entry: DeviceDirectoryEntry): void {
    this.workspacePickerDeviceId = entry.deviceId;
    this.showWorkspacePicker = true;
    this.onAddWorkspace(entry.deviceId);
  }

  private workspacePickerDevice(): DeviceDirectoryEntry {
    const entry = this.deviceEntries().find((item: DeviceDirectoryEntry): boolean =>
      item.deviceId === this.workspacePickerDeviceId
    );
    return entry || this.transientEntry;
  }

  private workspacePickerSheetOptions(): SheetOptions {
    return AdaptiveSheetOptions.fromPlacement(this.workspacePickerPlacement, true);
  }

  private visibleDeviceEntries(): DeviceDirectoryEntry[] {
    const entries = this.deviceEntries();
    return entries.length <= this.visibleDeviceCount ? entries : entries.slice(0, this.visibleDeviceCount);
  }

  private selectedEntry(): DeviceDirectoryEntry | undefined {
    const entries = this.deviceEntries();
    const selectedId = this.directoryState.selectedDeviceId.trim();
    const selected = entries.find((entry: DeviceDirectoryEntry): boolean => entry.deviceId === selectedId);
    if (selected) {
      return selected;
    }
    const activeId = this.controlTargetDeviceId.trim();
    return entries.find((entry: DeviceDirectoryEntry): boolean => entry.deviceId === activeId) || entries[0];
  }

  private isSelected(entry: DeviceDirectoryEntry): boolean {
    const selected = this.selectedEntry();
    return !!selected && selected.deviceId === entry.deviceId;
  }

  private deviceKey(entry: DeviceDirectoryEntry): string {
    return this.isDirectoryEntry(entry) ? entry.deviceId : `transient:${entry.deviceId}`;
  }

  private updateTransientEntry(): void {
    const targetId = this.controlTargetDeviceId.trim();
    this.transientEntry.deviceId = targetId;
    this.transientEntry.deviceName = this.controlTargetDeviceName.trim() || targetId;
    this.transientEntry.online = this.isConnected();
    this.transientEntry.status = 'ready';
    this.transientEntry.workspaces = this.recentWorkspaces;
    this.transientEntry.sessions = this.sessions;
  }

  private isDirectoryEntry(entry: DeviceDirectoryEntry): boolean {
    return this.directoryState.find(entry.deviceId) === entry;
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
    return entry.sessions.some((item: RemoteSession): boolean => item.id === this.selectedSessionId) ?
      this.selectedSessionId : '';
  }

  private isActiveDevice(deviceId: string): boolean {
    const activeId = this.controlTargetDeviceId;
    if (activeId.length === 0) {
      return this.directoryState.devices.length === 0;
    }
    return deviceId === activeId;
  }

  private canUseDevice(entry: DeviceDirectoryEntry): boolean {
    return entry.online || (this.isActiveDevice(entry.deviceId) && this.isConnected());
  }

  private isConnected(): boolean {
    return this.connectionState === 'connected' || this.connectionState === 'reconnecting';
  }
}
