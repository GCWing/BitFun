import { RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { DeviceDirectoryEntry } from '../state/DeviceDirectoryState';
import {
  SessionListInputs,
  SessionListProjection,
  SessionListProjectionCache
} from '../policy/SessionListProjection';
import { ConversationSessionFilterPolicy } from '../policy/ConversationSessionFilterPolicy';
import {
  SidebarDirectoryPreviewPolicy,
  SidebarPreviewCursor
} from '../policy/SidebarDirectoryPreviewPolicy';
import { GREEN, INK, MUTED, SOFT, SUBTLE } from './Theme';

/**
 * One desktop's workspaces and sessions.
 *
 * The projection cache is a single slot whose key includes the current
 * workspace path, so one instance cannot serve two devices without missing
 * every rebuild. This component exists so each desktop keeps its own cache.
 */
@ComponentV2
export struct SidebarDeviceGroup {
  @Param device: DeviceDirectoryEntry = new DeviceDirectoryEntry();
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
  @Local collapsedPaths: string[] = [];
  @Local expandedWorkspacePaths: string[] = [];
  @Local visibleWorkspaceCount: number = SidebarDirectoryPreviewPolicy.PREVIEW_COUNT;
  @Local sessionCursors: SidebarPreviewCursor[] = [];
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
            this.WorkspaceGroup(entry)
          }, (entry: RecentWorkspaceEntry): string => `${this.device.deviceId}:${entry.path}`)
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
        .fontSize(15)
        .fontWeight(FontWeight.Medium)
        .fontColor(this.device.online ? INK : SUBTLE)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .hitTestBehavior(HitTestMode.None)
      if (!this.device.online) {
        Text(RemoteI18n.t('sidebar.deviceOffline'))
          .fontSize(12)
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
        .fontSize(13)
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
        .fontSize(13)
        .fontColor(MUTED)
        .layoutWeight(1)
      Text(RemoteI18n.t('common.retry'))
        .fontSize(13)
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
      .fontSize(14)
      .fontColor(MUTED)
      .width('100%')
      .padding({ left: this.bodyIndent(), top: 6, bottom: 6 })
  }

  @Builder
  private WorkspaceGroup(entry: RecentWorkspaceEntry) {
    Column() {
      this.WorkspaceRow(entry)
      if (this.isWorkspaceExpanded(entry.path)) {
        ForEach(this.visibleSessionsFor(entry.path), (item: RemoteSession) => {
          this.SessionRow(item)
        }, (item: RemoteSession): string => `${item.deviceId || this.device.deviceId}:${item.id}`)
        if (this.hiddenSessionCount(entry.path) > 0) {
          this.MoreSessionsRow(entry.path)
        }
      }
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
    .margin({ bottom: 6 })
  }

  @Builder
  private WorkspaceRow(entry: RecentWorkspaceEntry) {
    Row({ space: 10 }) {
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph(this.isWorkspaceExpanded(entry.path) ?
          $r('sys.symbol.chevron_down') : $r('sys.symbol.chevron_right'))
          .fontSize(13)
          .fontColor([MUTED])
          .hitTestBehavior(HitTestMode.None)
      }
      .width(16)
      .height(18)
      SymbolGlyph($r('sys.symbol.folder'))
        .fontSize(19)
        .fontColor([this.isCurrentWorkspace(entry.path) ? INK : MUTED])
        .width(21)
        .height(21)
      Text(entry.name || this.basename(entry.path))
        .fontSize(15)
        .fontWeight(this.isCurrentWorkspace(entry.path) ? FontWeight.Medium : FontWeight.Regular)
        .fontColor(INK)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.plus'))
          .fontSize(17)
          .fontColor([INK])
      }
      .width(30)
      .height(40)
      .accessibilityText(RemoteI18n.t('sidebar.newInWorkspace'))
      .onClick(() => {
        this.onCreateInWorkspace(entry.path);
      })
    }
    .width('100%')
    .height(46)
    .padding({ left: this.bodyIndent(), right: 6 })
    .alignItems(VerticalAlign.Center)
    .borderRadius(10)
    .onClick(() => {
      this.toggleWorkspace(entry.path);
      this.onOpenWorkspace(entry.path);
    })
    .gesture(LongPressGesture({ repeat: false }).onAction(() => {
      this.onOpenWorkspace(entry.path);
    }))
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
        if (this.isActive && item.id === this.runningSessionId) {
          Text('')
            .width(7)
            .height(7)
            .backgroundColor(GREEN)
            .borderRadius(4)
        }
      }
      .width(19)
      .height(19)
      Text(item.title || RemoteI18n.t('sidebar.untitled'))
        .fontSize(15)
        .fontWeight(item.id === this.selectedSessionId ? FontWeight.Medium : FontWeight.Regular)
        .fontColor(INK)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .height(44)
    .padding({ left: this.bodyIndent() + 34, right: 10 })
    .alignItems(VerticalAlign.Center)
    .backgroundColor(item.id === this.selectedSessionId ? SOFT : '#00000000')
    .borderRadius(10)
    .onClick(() => {
      this.onOpenSession(item);
    })
    .gesture(LongPressGesture({ repeat: false }).onAction(() => {
      this.onSessionActions(item);
    }))
  }

  @Builder
  private MoreWorkspacesRow() {
    Row({ space: 8 }) {
      Text('···')
        .fontSize(13)
        .fontColor(MUTED)
      Text(RemoteI18n.f('sidebar.moreWorkspaces', String(this.hiddenWorkspaceCount())))
        .fontSize(13)
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

  @Builder
  private MoreSessionsRow(path: string) {
    Row({ space: 8 }) {
      Text('···')
        .fontSize(13)
        .fontColor(MUTED)
      Text(RemoteI18n.f('sidebar.moreSessions', String(this.hiddenSessionCount(path))))
        .fontSize(13)
        .fontColor(MUTED)
    }
    .width('100%')
    .height(40)
    .padding({ left: this.bodyIndent() + 34 })
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      this.revealMoreSessions(path);
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
    const entries = this.projection().projects.slice();
    if (this.workspacePath.length > 0 &&
      !entries.some((item: RecentWorkspaceEntry): boolean => item.path === this.workspacePath)) {
      entries.unshift({
        path: this.workspacePath,
        name: this.workspaceName,
        lastOpened: '',
        workspaceKind: this.workspaceKind
      });
    }
    this.recentWorkspaces.forEach((item: RecentWorkspaceEntry) => {
      if (item.path.length > 0 &&
        !entries.some((entry: RecentWorkspaceEntry): boolean => entry.path === item.path)) {
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

  private sessionVisibleCount(path: string): number {
    for (let index = 0; index < this.sessionCursors.length; index += 1) {
      if (this.sessionCursors[index].path === path) {
        return this.sessionCursors[index].visibleCount;
      }
    }
    return SidebarDirectoryPreviewPolicy.PREVIEW_COUNT;
  }

  private visibleSessionsFor(path: string): RemoteSession[] {
    return SidebarDirectoryPreviewPolicy.visibleSlice(
      this.sessionsFor(path),
      this.sessionVisibleCount(path)
    );
  }

  private hiddenSessionCount(path: string): number {
    return SidebarDirectoryPreviewPolicy.hiddenCount(
      this.sessionsFor(path).length,
      this.sessionVisibleCount(path)
    );
  }

  private revealMoreSessions(path: string): void {
    const next = SidebarDirectoryPreviewPolicy.nextVisibleCount(
      this.sessionVisibleCount(path),
      this.sessionsFor(path).length
    );
    const nextCursors: SidebarPreviewCursor[] = [];
    let replaced = false;
    for (let index = 0; index < this.sessionCursors.length; index += 1) {
      const cursor = this.sessionCursors[index];
      if (cursor.path === path) {
        nextCursors.push(new SidebarPreviewCursor(path, next));
        replaced = true;
      } else {
        nextCursors.push(cursor);
      }
    }
    if (!replaced) {
      nextCursors.push(new SidebarPreviewCursor(path, next));
    }
    this.sessionCursors = nextCursors;
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

  private isWorkspaceExpanded(path: string): boolean {
    if (SidebarDirectoryPreviewPolicy.workspaceExpandedByDefault(this.showHeader)) {
      return this.collapsedPaths.indexOf(path) < 0;
    }
    return this.expandedWorkspacePaths.indexOf(path) >= 0;
  }

  private toggleWorkspace(path: string): void {
    if (SidebarDirectoryPreviewPolicy.workspaceExpandedByDefault(this.showHeader)) {
      if (this.collapsedPaths.indexOf(path) >= 0) {
        this.collapsedPaths = this.collapsedPaths.filter((item: string) => item !== path);
        return;
      }
      this.collapsedPaths = this.collapsedPaths.concat([path]);
      return;
    }
    if (this.expandedWorkspacePaths.indexOf(path) >= 0) {
      this.expandedWorkspacePaths = this.expandedWorkspacePaths.filter((item: string) => item !== path);
      return;
    }
    this.expandedWorkspacePaths = this.expandedWorkspacePaths.concat([path]);
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
