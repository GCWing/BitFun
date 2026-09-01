import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import {
  DeviceDirectoryEntry,
  DeviceDirectoryState,
  WorkspaceDirectoryStatus
} from '../state/DeviceDirectoryState';
import {
  SessionListInputs,
  SessionListProjection,
  SessionListProjectionCache
} from '../policy/SessionListProjection';
import { ConversationSessionFilterPolicy } from '../policy/ConversationSessionFilterPolicy';
import { SidebarDirectoryPreviewPolicy } from '../policy/SidebarDirectoryPreviewPolicy';
import {
  INK,
  MUTED,
  SOFT,
  STATUS_SUCCESS,
  SUBTLE,
  TRANSPARENT,
} from './Theme';

/**
 * One keyed disclosure boundary per workspace. Disclosure and request state
 * live on DeviceDirectoryEntry so navigation can rebuild this component without
 * collapsing the branch or forgetting an in-flight lazy load.
 */
@ComponentV2
struct SidebarWorkspaceGroup {
  @Param deviceId: string = '';
  @Param path: string = '';
  @Param name: string = '';
  @Param sessions: RemoteSession[] = [];
  @Param selectedSessionId: string = '';
  @Param runningSessionId: string = '';
  @Param currentWorkspace: boolean = false;
  @Param activeDevice: boolean = false;
  @Param bodyIndent: number = 10;
  @Param expanded: boolean = false;
  @Param loadStatus: WorkspaceDirectoryStatus = 'idle';
  @Event onOpenWorkspace: (path: string) => void = (_path: string) => {};
  @Event onExpandWorkspace: (path: string) => Promise<void> = async (_path: string) => {};
  @Event onExpandedChange: (path: string, expanded: boolean) => void =
    (_path: string, _expanded: boolean) => {};
  @Event onLoadStatusChange: (path: string, status: WorkspaceDirectoryStatus) => void =
    (_path: string, _status: WorkspaceDirectoryStatus) => {};
  @Event onCreateInWorkspace: (path: string) => void = (_path: string) => {};
  @Event onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onSessionActions: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Local visibleSessionCount: number = SidebarDirectoryPreviewPolicy.PREVIEW_COUNT;

  @Monitor('sessions')
  onSessionsChanged(): void {
    if (this.sessions.length > 0) {
      this.onLoadStatusChange(this.path, 'ready');
    }
  }

  build() {
    Column() {
      this.WorkspaceRow()
      if (this.expanded) {
        if (this.loadStatus === 'loading') {
          this.SessionLoadingRow()
        } else if (this.loadStatus === 'failed') {
          this.SessionRetryRow()
        }
        ForEach(this.visibleSessions(), (item: RemoteSession) => {
          this.SessionRow(item)
        }, (item: RemoteSession): string => `${item.deviceId || this.deviceId}:${item.id}`)
        if (this.hiddenSessionCount() > 0) {
          this.MoreSessionsRow()
        }
      }
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
    .margin({ bottom: 6 })
  }

  @Builder
  private WorkspaceRow() {
    Row({ space: 6 }) {
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph(this.expanded ?
          $r('sys.symbol.chevron_down') : $r('sys.symbol.chevron_right'))
          .fontSize(13)
          .fontColor([MUTED])
          .hitTestBehavior(HitTestMode.None)
      }
      .width(24)
      .height(46)
      .accessibilityText(this.expanded ?
        RemoteI18n.t('chat.showLess') : RemoteI18n.t('chat.showMore'))
      .onClick(() => {
        void this.toggleExpanded();
      })

      Row({ space: 10 }) {
        SymbolGlyph($r('sys.symbol.folder'))
          .fontSize(19)
          .fontColor([this.currentWorkspace ? INK : MUTED])
          .width(21)
          .height(21)
          .hitTestBehavior(HitTestMode.None)
        Text(this.name || this.basename(this.path))
          .fontSize(MobileDesignTypography.titleSmall.size)
          .fontWeight(this.currentWorkspace ? FontWeight.Medium : FontWeight.Regular)
          .fontColor(INK)
          .layoutWeight(1)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .hitTestBehavior(HitTestMode.None)
      }
      .layoutWeight(1)
      .height(46)
      .alignItems(VerticalAlign.Center)
      .onClick(() => {
        void this.toggleExpanded();
      })
      .gesture(LongPressGesture({ repeat: false }).onAction(() => {
        this.onOpenWorkspace(this.path);
      }))

      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.plus'))
          .fontSize(17)
          .fontColor([INK])
          .hitTestBehavior(HitTestMode.None)
      }
      .width(30)
      .height(40)
      .accessibilityText(RemoteI18n.t('sidebar.newInWorkspace'))
      .onClick(() => {
        this.onCreateInWorkspace(this.path);
      })
    }
    .width('100%')
    .height(46)
    .padding({ left: Math.max(0, this.bodyIndent - 4), right: 6 })
    .alignItems(VerticalAlign.Center)
    .borderRadius(10)
  }

  @Builder
  private SessionLoadingRow() {
    Row({ space: 8 }) {
      LoadingProgress()
        .width(13)
        .height(13)
        .color(MUTED)
      Text(RemoteI18n.t('common.loading'))
        .fontSize(MobileDesignTypography.bodySmall.size)
        .fontColor(MUTED)
    }
    .width('100%')
    .height(40)
    .padding({ left: this.bodyIndent + 34, right: 10 })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private SessionRetryRow() {
    Row({ space: 8 }) {
      Text(RemoteI18n.t('sidebar.deviceLoadFailed'))
        .fontSize(MobileDesignTypography.bodySmall.size)
        .fontColor(MUTED)
        .layoutWeight(1)
      Text(RemoteI18n.t('common.retry'))
        .fontSize(MobileDesignTypography.bodySmall.size)
        .fontColor(INK)
    }
    .width('100%')
    .height(40)
    .padding({ left: this.bodyIndent + 34, right: 10 })
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      void this.loadSessions();
    })
  }

  @Builder
  private SessionRow(item: RemoteSession) {
    Row({ space: 8 }) {
      Stack({ alignContent: Alignment.BottomStart }) {
        SymbolGlyph(this.sessionGlyph(item))
          .fontSize(17)
          .fontColor([MUTED])
          .width(19)
          .height(19)
        if (this.activeDevice && item.id === this.runningSessionId) {
          Text('')
            .width(7)
            .height(7)
            .backgroundColor(STATUS_SUCCESS)
            .borderRadius(4)
        }
      }
      .width(19)
      .height(19)
      Text(item.title || RemoteI18n.t('sidebar.untitled'))
        .fontSize(MobileDesignTypography.titleSmall.size)
        .fontWeight(item.id === this.selectedSessionId ? FontWeight.Medium : FontWeight.Regular)
        .fontColor(INK)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .height(44)
    .padding({ left: this.bodyIndent + 34, right: 10 })
    .alignItems(VerticalAlign.Center)
    .backgroundColor(item.id === this.selectedSessionId ? SOFT : TRANSPARENT)
    .borderRadius(10)
    .onClick(() => {
      this.onOpenSession(item);
    })
    .gesture(LongPressGesture({ repeat: false }).onAction(() => {
      this.onSessionActions(item);
    }))
  }

  @Builder
  private MoreSessionsRow() {
    Row({ space: 8 }) {
      Text('···')
        .fontSize(MobileDesignTypography.bodySmall.size)
        .fontColor(MUTED)
      Text(RemoteI18n.f('sidebar.moreSessions', String(this.hiddenSessionCount())))
        .fontSize(MobileDesignTypography.bodySmall.size)
        .fontColor(MUTED)
    }
    .width('100%')
    .height(40)
    .padding({ left: this.bodyIndent + 34 })
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      this.visibleSessionCount = SidebarDirectoryPreviewPolicy.nextVisibleCount(
        this.visibleSessionCount,
        this.sessions.length
      );
    })
  }

  private visibleSessions(): RemoteSession[] {
    return SidebarDirectoryPreviewPolicy.visibleSlice(this.sessions, this.visibleSessionCount);
  }

  private async toggleExpanded(): Promise<void> {
    const expanded = !this.expanded;
    this.onExpandedChange(this.path, expanded);
    if (!expanded || this.sessions.length > 0 || this.loadStatus === 'ready' ||
      this.loadStatus === 'loading') {
      return;
    }
    await this.loadSessions();
  }

  private async loadSessions(): Promise<void> {
    if (this.loadStatus === 'loading') {
      return;
    }
    this.onLoadStatusChange(this.path, 'loading');
    try {
      await this.onExpandWorkspace(this.path);
      this.onLoadStatusChange(this.path, 'ready');
    } catch (_err) {
      // Keep failure local to this branch; the retry row is answerable without
      // collapsing it and sibling disclosure state stays untouched.
      this.onLoadStatusChange(this.path, 'failed');
    }
  }

  private hiddenSessionCount(): number {
    return SidebarDirectoryPreviewPolicy.hiddenCount(this.sessions.length, this.visibleSessionCount);
  }

  private sessionGlyph(item: RemoteSession): Resource {
    const agentType = (item.agentType ? item.agentType : '').toLowerCase();
    if (agentType === 'code') {
      return $r('sys.symbol.code_square');
    }
    if (agentType === 'claw' || agentType === 'assistant' || agentType === 'chat') {
      return $r('sys.symbol.message');
    }
    return $r('sys.symbol.doc_text');
  }

  private basename(path: string): string {
    const normalized = path.replace(/\/+$/, '');
    const index = normalized.lastIndexOf('/');
    return index >= 0 ? normalized.substring(index + 1) : normalized;
  }
}

/**
 * One desktop's workspaces and sessions.
 *
 * The projection cache is a single slot whose key includes the current
 * workspace path, so one instance cannot serve two devices without missing
 * every rebuild. Its parent therefore keys this component boundary by device.
 */
@ComponentV2
export struct SidebarDeviceGroup {
  @Param device: DeviceDirectoryEntry = new DeviceDirectoryEntry();
  @Param directoryState: DeviceDirectoryState = new DeviceDirectoryState();
  @Param sessions: RemoteSession[] = [];
  @Param recentWorkspaces: RecentWorkspaceEntry[] = [];
  @Param workspaceName: string = '';
  @Param workspacePath: string = '';
  @Param workspaceKind: string = 'normal';
  @Param selectedSessionId: string = '';
  @Param runningSessionId: string = '';
  @Param query: string = '';
  @Param showHeader: boolean = false;
  @Param isActive: boolean = false;
  @Event onToggle: () => void = () => {};
  @Event onRetry: () => void = () => {};
  @Event onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onSessionActions: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onCreateInWorkspace: (path: string) => void = (_path: string) => {};
  @Event onOpenWorkspace: (path: string) => void = (_path: string) => {};
  @Event onExpandWorkspace: (path: string) => Promise<void> = async (_path: string) => {};
  @Event onWorkspaceExpandedChange: (path: string, expanded: boolean) => void =
    (_path: string, _expanded: boolean) => {};
  @Event onWorkspaceLoadStatusChange: (path: string, status: WorkspaceDirectoryStatus) => void =
    (_path: string, _status: WorkspaceDirectoryStatus) => {};
  @Local visibleWorkspaceCount: number = SidebarDirectoryPreviewPolicy.PREVIEW_COUNT;
  private readonly projectionCache: SessionListProjectionCache = new SessionListProjectionCache();

  build() {
    Column() {
      if (this.showHeader) {
        this.DeviceHeader()
      }
      if (this.shouldShowBody()) {
        if (this.device.status === 'failed') {
          this.FailedRow()
        } else if (this.workspaceEntries().length === 0) {
          if (this.device.status === 'ready') {
            this.EmptyWorkspaces()
          } else if (this.device.status === 'cached') {
            this.EmptyWorkspaces()
          } else {
            this.LoadingRow()
          }
        } else {
          ForEach(this.visibleWorkspaceEntries(), (entry: RecentWorkspaceEntry) => {
            SidebarWorkspaceGroup({
              deviceId: this.device.deviceId,
              path: entry.path,
              name: entry.name,
              sessions: this.sessionsFor(entry.path),
              selectedSessionId: this.selectedSessionId,
              runningSessionId: this.runningSessionId,
              currentWorkspace: this.isCurrentWorkspace(entry.path),
              activeDevice: this.isActive,
              bodyIndent: this.bodyIndent(),
              expanded: this.directoryState.workspaceExpanded(this.device.deviceId, entry.path),
              loadStatus: this.sessionsFor(entry.path).length > 0 ?
                'ready' : this.directoryState.workspaceStatus(this.device.deviceId, entry.path),
              onOpenWorkspace: this.onOpenWorkspace,
              onExpandWorkspace: this.onExpandWorkspace,
              onExpandedChange: this.onWorkspaceExpandedChange,
              onLoadStatusChange: this.onWorkspaceLoadStatusChange,
              onCreateInWorkspace: this.onCreateInWorkspace,
              onOpenSession: this.onOpenSession,
              onSessionActions: this.onSessionActions
            })
          }, (entry: RecentWorkspaceEntry): string =>
            SidebarDirectoryPreviewPolicy.workspaceKey(this.device.deviceId, entry.path))
          if (this.hiddenWorkspaceCount() > 0) {
            this.MoreWorkspacesRow()
          }
          if (this.device.status === 'loading') {
            this.LoadingRow()
          }
        }
      }
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
    .margin({ bottom: this.showHeader ? 8 : 0 })
  }

  @Builder
  private DeviceHeader() {
    Row({ space: 10 }) {
      SymbolGlyph($r('sys.symbol.desktop'))
        .fontSize(19)
        .fontColor([this.device.online ? INK : SUBTLE])
        .width(21)
        .height(21)
        .hitTestBehavior(HitTestMode.None)
      Text(this.device.deviceName)
        .fontSize(MobileDesignTypography.titleSmall.size)
        .fontWeight(FontWeight.Medium)
        .fontColor(this.device.online ? INK : SUBTLE)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .hitTestBehavior(HitTestMode.None)
      if (!this.device.online) {
        Text(RemoteI18n.t('sidebar.deviceOffline'))
          .fontSize(MobileDesignTypography.labelSmall.size)
          .fontColor(SUBTLE)
          .hitTestBehavior(HitTestMode.None)
      }
      if (this.device.status === 'loading') {
        LoadingProgress()
          .width(14)
          .height(14)
          .color(MUTED)
          .hitTestBehavior(HitTestMode.None)
      }
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph(this.device.expanded ?
          $r('sys.symbol.chevron_down') : $r('sys.symbol.chevron_right'))
          .fontSize(13)
          .fontColor([MUTED])
          .hitTestBehavior(HitTestMode.None)
      }
      .width(16)
      .height(16)
      .hitTestBehavior(HitTestMode.None)
    }
    .width('100%')
    .height(46)
    .padding({ left: 10, right: 6 })
    .alignItems(VerticalAlign.Center)
    .opacity(this.device.online ? 1 : 0.7)
    .onClick(() => {
      this.onToggle();
    })
  }

  @Builder
  private LoadingRow() {
    Row({ space: 8 }) {
      LoadingProgress()
        .width(14)
        .height(14)
        .color(MUTED)
      Text(RemoteI18n.t('common.loading'))
        .fontSize(MobileDesignTypography.bodySmall.size)
        .fontColor(MUTED)
    }
    .width('100%')
    .height(40)
    .padding({ left: this.bodyIndent(), right: 10 })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private FailedRow() {
    Row({ space: 8 }) {
      Text(this.device.errorText.length > 0 ?
        this.device.errorText : RemoteI18n.t('sidebar.deviceLoadFailed'))
        .fontSize(MobileDesignTypography.bodySmall.size)
        .fontColor(MUTED)
        .layoutWeight(1)
      Text(RemoteI18n.t('common.retry'))
        .fontSize(MobileDesignTypography.bodySmall.size)
        .fontColor(INK)
        .onClick(() => {
          this.onRetry();
        })
    }
    .width('100%')
    .height(40)
    .padding({ left: this.bodyIndent(), right: 10 })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private EmptyWorkspaces() {
    Text(RemoteI18n.t('sidebar.emptyWorkspaces'))
      .fontSize(MobileDesignTypography.bodyMedium.size)
      .fontColor(MUTED)
      .width('100%')
      .padding({ left: this.bodyIndent(), top: 6, bottom: 6 })
  }

  @Builder
  private MoreWorkspacesRow() {
    Row({ space: 8 }) {
      Text('···')
        .fontSize(MobileDesignTypography.bodySmall.size)
        .fontColor(MUTED)
      Text(RemoteI18n.f('sidebar.moreWorkspaces', String(this.hiddenWorkspaceCount())))
        .fontSize(MobileDesignTypography.bodySmall.size)
        .fontColor(MUTED)
    }
    .width('100%')
    .height(40)
    .padding({ left: this.bodyIndent() })
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      this.visibleWorkspaceCount = SidebarDirectoryPreviewPolicy.nextVisibleCount(
        this.visibleWorkspaceCount,
        this.workspaceEntries().length
      );
    })
  }

  private shouldShowBody(): boolean {
    if (!this.showHeader) {
      return true;
    }
    if (!this.device.online && this.device.workspaces.length === 0 &&
      this.recentWorkspaces.length === 0) {
      return false;
    }
    return this.device.expanded;
  }

  private bodyIndent(): number {
    return this.showHeader ? 20 : 10;
  }

  private projection(): SessionListProjection {
    const inputs: SessionListInputs = {
      sessions: this.sessions,
      query: this.query,
      sortMode: 'project',
      workspaceName: this.workspaceName,
      workspacePath: this.workspacePath,
      workspaceKind: this.workspaceKind,
      recentWorkspaces: this.recentWorkspaces,
      workspaceFilter: '',
      agentFilter: '',
      statusFilter: '',
      showWorkspaceMetadata: false,
      showUpdatedMetadata: false,
      showStatusMetadata: false
    };
    return this.projectionCache.get(inputs);
  }

  private workspaceEntries(): RecentWorkspaceEntry[] {
    // The shared projection intentionally removes assistant workspaces because
    // the desktop conversation sidebar presents those sessions as top-level
    // chats. The device directory has a different information architecture:
    // every session belongs below its workspace, including Claw/assistant
    // workspaces. Reintroduce the complete device catalog here without changing
    // the shared desktop projection contract.
    const entries: RecentWorkspaceEntry[] = [];
    this.projection().projects.forEach((item: RecentWorkspaceEntry): void => {
      if (!entries.some((entry: RecentWorkspaceEntry): boolean =>
        ConversationSessionFilterPolicy.workspacePathsEqual(entry.path, item.path))) {
        entries.push(item);
      }
    });
    if (this.workspacePath.length > 0 &&
      !entries.some((item: RecentWorkspaceEntry): boolean =>
        ConversationSessionFilterPolicy.workspacePathsEqual(item.path, this.workspacePath))) {
      entries.unshift({
        path: this.workspacePath,
        name: this.workspaceName,
        lastOpened: '',
        workspaceKind: this.workspaceKind
      });
    }
    this.recentWorkspaces.forEach((item: RecentWorkspaceEntry) => {
      if (item.path.length > 0 &&
        !entries.some((entry: RecentWorkspaceEntry): boolean =>
          ConversationSessionFilterPolicy.workspacePathsEqual(entry.path, item.path))) {
        entries.push(item);
      }
    });
    const query = this.query.trim().toLowerCase();
    if (query.length === 0) {
      return entries;
    }
    return entries.filter((entry: RecentWorkspaceEntry): boolean => {
      const label = (entry.name || this.basename(entry.path)).toLowerCase();
      return label.indexOf(query) >= 0 || entry.path.toLowerCase().indexOf(query) >= 0 ||
        this.sessionsFor(entry.path).length > 0;
    });
  }

  private visibleWorkspaceEntries(): RecentWorkspaceEntry[] {
    return SidebarDirectoryPreviewPolicy.visibleSlice(
      this.workspaceEntries(),
      this.visibleWorkspaceCount
    );
  }

  private hiddenWorkspaceCount(): number {
    return SidebarDirectoryPreviewPolicy.hiddenCount(
      this.workspaceEntries().length,
      this.visibleWorkspaceCount
    );
  }

  private sessionsFor(path: string): RemoteSession[] {
    const target = ConversationSessionFilterPolicy.normalizeWorkspacePath(path);
    return this.projection().filtered.filter((item: RemoteSession): boolean => {
      const itemPath = item.workspacePath ? item.workspacePath : this.workspacePath;
      return ConversationSessionFilterPolicy.normalizeWorkspacePath(itemPath) === target;
    });
  }

  private isCurrentWorkspace(path: string): boolean {
    return this.isActive && path.length > 0 && path === this.workspacePath;
  }

  private basename(path: string): string {
    const normalized = path.replace(/\/+$/, '');
    const index = normalized.lastIndexOf('/');
    return index >= 0 ? normalized.substring(index + 1) : normalized;
  }

}
