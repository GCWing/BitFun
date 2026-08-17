import { RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { GREEN, INK, MUTED, SOFT, SUBTLE } from './Theme';
import {
  SessionListInputs,
  SessionListProjection,
  SessionListProjectionCache,
  SessionListProjector
} from '../policy/SessionListProjection';

/**
 * How many sessions a workspace shows before it hands the rest to an overflow
 * row. Three is what fits under a workspace on a phone without pushing the next
 * workspace off screen, and the overflow row states the real total — a scroll
 * bar cannot tell you a workspace holds 119 sessions, this can.
 */
const SESSIONS_PER_WORKSPACE: number = 3;
const WORKSPACE_BATCH: number = 3;

/**
 * The workspace half of the unified sidebar: every workspace the desktop knows
 * about, each with its own sessions inlined beneath it.
 *
 * This exists because "local" and "remote" were never two comparable places.
 * Remote carries a context — which machine, which workspace, connected or not —
 * and that context needs somewhere to live; a flat conversation list has no
 * room for it. So conversations stay a timeline above, and workspaces are their
 * own section here, rather than the two being pitted against each other in a
 * segmented control where one side was always going to look empty.
 */
@ComponentV2
export struct SidebarWorkspaceSection {
  @Param sessions: RemoteSession[] = [];
  @Param recentWorkspaces: RecentWorkspaceEntry[] = [];
  @Param workspaceName: string = '';
  @Param workspacePath: string = '';
  @Param workspaceKind: string = 'normal';
  @Param connectionState: string = 'idle';
  @Param selectedSessionId: string = '';
  @Param runningSessionId: string = '';
  @Param query: string = '';
  @Event onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onSessionActions: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onCreateInWorkspace: (path: string) => void = (_path: string) => {};
  @Event onOpenWorkspace: (path: string) => void = (_path: string) => {};
  /**
   * Opens the connect sheet. Not "go to the remote surface": there is nothing
   * to go to while offline, and a row reading "connect a desktop" that lands on
   * an empty placeholder is the same as a row that does nothing.
   */
  @Event onConnect: () => void = () => {};
  @Local collapsedPaths: string[] = [];
  @Local expandedSessionPaths: string[] = [];
  @Local visibleWorkspaceCount: number = WORKSPACE_BATCH;
  private readonly projectionCache: SessionListProjectionCache = new SessionListProjectionCache();

  build() {
    Column() {
      this.SectionHeader()
      if (!this.isConnected()) {
        this.ConnectRow()
      } else if (this.workspaceEntries().length === 0) {
        this.EmptyWorkspaces()
      } else {
        ForEach(this.visibleWorkspaceEntries(), (entry: RecentWorkspaceEntry) => {
          this.WorkspaceGroup(entry)
        }, (entry: RecentWorkspaceEntry): string => entry.path)
        if (this.visibleWorkspaceCount < this.workspaceEntries().length) {
          this.MoreWorkspacesRow()
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
      Text(RemoteI18n.t('sidebar.workspaces'))
        .id('sidebar-workspace-section')
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
      Blank()
      // Offline is a property of the workspaces, not of the app, so it is
      // stated here rather than as a global connection banner.
      if (!this.isConnected()) {
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
      // Workspaces are whatever the desktop reports; the phone cannot create
      // one. So this "+" adds a *connection*, and says so — labelling it "add
      // workspace" would promise a capability that does not exist.
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
  private EmptyWorkspaces() {
    Text(RemoteI18n.t('sidebar.emptyWorkspaces'))
      .fontSize(14)
      .fontColor(MUTED)
      .width('100%')
      .padding({ left: 10, top: 6, bottom: 6 })
  }

  @Builder
  private WorkspaceGroup(entry: RecentWorkspaceEntry) {
    Column() {
      this.WorkspaceRow(entry)
      if (!this.isCollapsed(entry.path)) {
        ForEach(this.visibleSessionsFor(entry.path), (item: RemoteSession) => {
          this.SessionRow(item)
        }, (item: RemoteSession): string => item.id)
        if (this.visibleSessionLimit(entry.path) < this.sessionsFor(entry.path).length) {
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
        SymbolGlyph($r('sys.symbol.square_and_pencil'))
          .fontSize(16)
          .fontColor([MUTED])
      }
      .width(30)
      .height(40)
      .opacity(0.6)
      .accessibilityText(RemoteI18n.t('sidebar.newInWorkspace'))
      .onClick(() => {
        this.onCreateInWorkspace(entry.path);
      })
      SymbolGlyph(this.isCollapsed(entry.path) ?
        $r('sys.symbol.chevron_right') : $r('sys.symbol.chevron_down'))
        .fontSize(13)
        .fontColor([MUTED])
        .width(16)
        .height(16)
    }
    .width('100%')
    .height(46)
    .padding({ left: 10, right: 6 })
    .alignItems(VerticalAlign.Center)
    .borderRadius(10)
    .onClick(() => {
      this.toggleWorkspace(entry.path);
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
        // A running turn is worth a dot here: on a phone you fire a task and
        // switch away, and the sidebar is where you come back to look.
        if (item.id === this.runningSessionId) {
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
    .padding({ left: 26, right: 10 })
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
    .padding({ left: 26 })
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      this.expandedSessionPaths = this.expandedSessionPaths.concat([path]);
    })
  }

  @Builder
  private MoreWorkspacesRow() {
    Row({ space: 8 }) {
      Text('···')
        .fontSize(13)
        .fontColor(MUTED)
      Text(RemoteI18n.f('sidebar.moreWorkspaces',
        String(this.workspaceEntries().length - this.visibleWorkspaceCount)))
        .fontSize(13)
        .fontColor(MUTED)
    }
    .width('100%')
    .height(40)
    .padding({ left: 10 })
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      this.visibleWorkspaceCount += WORKSPACE_BATCH;
    })
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

  /**
   * Only workspaces that are somewhere you can go.
   *
   * `syncRemotePageSummary` seeds the current workspace with the placeholder
   * name 「未连接」 and no path, and the projection turns any named current
   * workspace into an entry. On the Remote surface that never showed, because
   * you only got there connected; in the sidebar it is on screen from launch,
   * so a reconnecting phone with no workspace data yet listed a folder called
   * 「未连接」 that opens nothing.
   */
  private workspaceEntries(): RecentWorkspaceEntry[] {
    return this.projection().projects.filter((entry: RecentWorkspaceEntry) => entry.path.length > 0);
  }

  private visibleWorkspaceEntries(): RecentWorkspaceEntry[] {
    const entries = this.workspaceEntries();
    return entries.length <= this.visibleWorkspaceCount ?
      entries : entries.slice(0, this.visibleWorkspaceCount);
  }

  private sessionsFor(path: string): RemoteSession[] {
    return SessionListProjector.sessionsForProject(this.projection(), path);
  }

  private visibleSessionLimit(path: string): number {
    return this.expandedSessionPaths.indexOf(path) >= 0 ?
      this.sessionsFor(path).length : SESSIONS_PER_WORKSPACE;
  }

  private visibleSessionsFor(path: string): RemoteSession[] {
    const bucket = this.sessionsFor(path);
    const limit = this.visibleSessionLimit(path);
    return bucket.length <= limit ? bucket : bucket.slice(0, limit);
  }

  private hiddenSessionCount(path: string): number {
    return this.sessionsFor(path).length - this.visibleSessionLimit(path);
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

  private isCollapsed(path: string): boolean {
    return this.collapsedPaths.indexOf(path) >= 0;
  }

  private toggleWorkspace(path: string): void {
    if (this.isCollapsed(path)) {
      this.collapsedPaths = this.collapsedPaths.filter((item: string) => item !== path);
      return;
    }
    this.collapsedPaths = this.collapsedPaths.concat([path]);
  }

  private isCurrentWorkspace(path: string): boolean {
    return path.length > 0 && path === this.workspacePath;
  }

  private isConnected(): boolean {
    return this.connectionState === 'connected' || this.connectionState === 'reconnecting';
  }

  private basename(path: string): string {
    const normalized = path.replace(/\/+$/, '');
    const index = normalized.lastIndexOf('/');
    return index >= 0 ? normalized.substring(index + 1) : normalized;
  }
}
