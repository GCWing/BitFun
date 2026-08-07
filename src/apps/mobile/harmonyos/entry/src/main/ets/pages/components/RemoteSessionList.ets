import { RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { TimeFormat } from '../../services/TimeFormat';
import { CARD, INK, MUTED, SOFT } from './Theme';
import { SessionActionPresentation, SessionActionSurface } from './SessionActionSurface';
import { SessionActionCapabilities, SessionActionPolicy, SessionActionScope } from '../state/SessionActionPolicy';
import { SessionDetailsView } from './SessionDetailsView';
import { ConversationSessionFilterPolicy } from '../state/ConversationSessionFilterPolicy';

@ComponentV2
export struct RemoteSessionList {
  @Param sessions: RemoteSession[] = [];
  @Param query: string = '';
  @Param sortMode: string = 'project';
  @Param hasMoreSessions: boolean = false;
  @Param isBusy: boolean = false;
  @Param selectedSessionId: string = '';
  @Param workspaceName: string = '';
  @Param workspacePath: string = '';
  @Param workspaceKind: string = 'normal';
  @Param recentWorkspaces: RecentWorkspaceEntry[] = [];
  @Param actionPresentation: SessionActionPresentation = SessionActionPresentation.BottomSheet;
  @Param workspaceFilter: string = '';
  @Param agentFilter: string = '';
  @Param statusFilter: string = '';
  @Param showWorkspaceMetadata: boolean = false;
  @Param showUpdatedMetadata: boolean = false;
  @Param showStatusMetadata: boolean = false;
  @Event onCreate: () => void = () => {};
  @Event onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onDeleteSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onLoadMore: () => void = () => {};
  @Event onCreateAssistantSession: () => void = () => {};
  @Event onCreateInWorkspace: (path: string, agentType: string) => void = (_path: string, _agentType: string) => {};
  @Event onSelectWorkspace: (path: string) => void = (_path: string) => {};
  @Local collapsedWorkspacePaths: string[] = [];
  @Local projectVisibleSteps: string[] = [];
  @Local visibleProjectCount: number = 3;
  @Local chatsCollapsed: boolean = false;
  @Local chatVisibleCount: number = 3;
  @Local todayCollapsed: boolean = false;
  @Local yesterdayCollapsed: boolean = false;
  @Local earlierCollapsed: boolean = false;
  @Local createMenuPath: string = '';
  @Local activeActionSessionId: string = '';
  @Local showSessionActionSheet: boolean = false;
  @Local detailsSessionId: string = '';
  @Local showSessionDetails: boolean = false;

  @Monitor('isBusy', 'workspacePath')
  onWorkspaceContextChanged(): void {
    this.createMenuPath = '';
  }

  build() {
    Column() {
      Scroll() {
        Column() {
          if (this.filteredSessions().length === 0) {
            if (this.hasActiveListFilter()) {
              this.FilteredEmptySessions()
            } else {
              this.EmptySessions()
            }
          } else if (this.sortMode === 'time') {
            this.TimeSection()
          } else if (this.sortMode === 'chat') {
            if (this.visibleChatSessions().length > 0) {
              this.ChatSection()
            }
            if (this.projectEntries().length > 0) {
              this.ProjectSection()
            }
          } else {
            if (this.projectEntries().length > 0) {
              this.ProjectSection()
            }
            if (this.visibleChatSessions().length > 0) {
              this.ChatSection()
            }
          }
        }
        .width('100%')
        .alignItems(HorizontalAlign.Start)
      }
      .layoutWeight(1)
      .width('100%')
      .align(Alignment.TopStart)
      .scrollable(ScrollDirection.Vertical)
      .scrollBar(BarState.Off)
    }
    .width('100%')
    .layoutWeight(1)
    .alignItems(HorizontalAlign.Start)
    .bindSheet($$this.showSessionActionSheet, this.SessionActionSheet(), this.sessionActionSheetOptions())
    .bindSheet($$this.showSessionDetails, this.SessionDetailsSheet(), this.sessionDetailsSheetOptions())
  }

  @Builder
  private TimeSection() {
    Column() {
      if (this.sessionsByTime().length === 0) {
        this.EmptySessions()
      } else {
        if (this.sessionsForTimeBucket('today').length > 0) {
          this.TodayGroup()
        }
        if (this.sessionsForTimeBucket('yesterday').length > 0) {
          this.YesterdayGroup()
        }
        if (this.sessionsForTimeBucket('earlier').length > 0) {
          this.EarlierGroup()
        }
      }
    }
    .width('100%')
    .margin({ top: 10, bottom: 16 })
  }

  @Builder
  private TodayGroup() {
    Column() {
      this.TimeGroupHeader(RemoteI18n.t('remote.time.today'), this.todayCollapsed, () => {
        this.todayCollapsed = !this.todayCollapsed;
      })
      if (!this.todayCollapsed) {
        ForEach(this.sessionsForTimeBucket('today'), (item: RemoteSession) => {
          this.SessionRow(item)
        }, (item: RemoteSession) => item.id)
      }
    }
    .width('100%')
    .margin({ bottom: 8 })
  }

  @Builder
  private YesterdayGroup() {
    Column() {
      this.TimeGroupHeader(RemoteI18n.t('remote.time.yesterday'), this.yesterdayCollapsed, () => {
        this.yesterdayCollapsed = !this.yesterdayCollapsed;
      })
      if (!this.yesterdayCollapsed) {
        ForEach(this.sessionsForTimeBucket('yesterday'), (item: RemoteSession) => {
          this.SessionRow(item)
        }, (item: RemoteSession) => item.id)
      }
    }
    .width('100%')
    .margin({ bottom: 8 })
  }

  @Builder
  private EarlierGroup() {
    Column() {
      this.TimeGroupHeader(RemoteI18n.t('remote.time.earlier'), this.earlierCollapsed, () => {
        this.earlierCollapsed = !this.earlierCollapsed;
      })
      if (!this.earlierCollapsed) {
        ForEach(this.sessionsForTimeBucket('earlier'), (item: RemoteSession) => {
          this.SessionRow(item)
        }, (item: RemoteSession) => item.id)
      }
    }
    .width('100%')
    .margin({ bottom: 8 })
  }

  @Builder
  private TimeGroupHeader(title: string, collapsed: boolean, action: () => void) {
    Row({ space: 8 }) {
      Text(title)
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
      if (collapsed) {
        SymbolGlyph($r('sys.symbol.chevron_right'))
          .fontSize(14)
          .fontColor([MUTED])
          .opacity(0.55)
      } else {
        SymbolGlyph($r('sys.symbol.chevron_down'))
          .fontSize(14)
          .fontColor([MUTED])
          .opacity(0.55)
      }
    }
    .width('100%')
    .height(42)
    .alignItems(VerticalAlign.Center)
    .onClick(action)
  }

  @Builder
  private ProjectSection() {
    Column({ space: 4 }) {
      Row() {
        Row({ space: 8 }) {
          Text(RemoteI18n.t('sidebar.projects'))
            .fontSize(14)
            .fontWeight(FontWeight.Medium)
            .fontColor(MUTED)
          Text(`${this.projectEntries().length}`)
            .fontSize(12)
            .fontColor(MUTED)
        }
        Blank()
      }
      .width('100%')
      .height(44)
      .alignItems(VerticalAlign.Center)
      ForEach(this.visibleProjectEntries(), (project: RecentWorkspaceEntry) => {
        Column({ space: 2 }) {
          Row({ space: 10 }) {
            SymbolGlyph($r('sys.symbol.folder'))
              .fontSize(22)
              .fontColor([INK])
              .width(24)
              .height(24)
            Text(project.name || this.basename(project.path))
              .fontSize(15)
              .fontColor(INK)
              .layoutWeight(1)
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
            if (this.isWorkspaceCollapsed(project.path)) {
              SymbolGlyph($r('sys.symbol.chevron_right'))
                .fontSize(14)
                .fontColor([MUTED])
                .width(18)
                .height(18)
            } else {
              SymbolGlyph($r('sys.symbol.chevron_down'))
                .fontSize(14)
                .fontColor([MUTED])
                .width(18)
                .height(18)
            }
            if (project.path.length > 0) {
              SymbolGlyph($r('sys.symbol.square_and_pencil'))
                .fontSize(18)
                .fontColor([MUTED])
                .width(22)
                .height(22)
                .opacity(0.52)
                .bindPopup(this.createMenuPath.length > 0 && this.createMenuPath === project.path, {
                  builder: () => {
                    this.ProjectCreateMenu(project.path)
                  },
                  placement: Placement.Top,
                  popupColor: '#00000000',
                  enableArrow: false,
                  autoCancel: true,
                  mask: false,
                  targetSpace: 6,
                  onStateChange: (event) => {
                    if (!event.isVisible) {
                      this.createMenuPath = '';
                    }
                  }
                })
                .onClick(() => {
                  this.createMenuPath = this.createMenuPath === project.path ? '' : project.path;
                })
            }
          }
          .width('100%')
          .height(44)
          .onClick(() => {
            this.toggleWorkspace(project.path);
          })
          if (!this.isWorkspaceCollapsed(project.path)) {
            this.projectPreview(project.path)
          }
        }
        .width('100%')
        .margin({ bottom: 8 })
      }, (project: RecentWorkspaceEntry): string => project.path)
      if (this.visibleProjectCount < this.projectEntries().length) {
        Text(RemoteI18n.f('remote.projects.showMore', String(this.nextProjectEntryBatchSize())))
          .fontSize(13)
          .fontColor(MUTED)
          .height(40)
          .width('100%')
          .textAlign(TextAlign.Center)
          .onClick(() => {
            this.visibleProjectCount += 3;
          })
      }
    }
    .width('100%')
    .margin({ top: 12 })
  }

  @Builder
  private ProjectCreateMenu(path: string) {
    Column({ space: 2 }) {
      this.ProjectCreateMenuItem('Code', 'code', path)
      this.ProjectCreateMenuItem('Cowork', 'Cowork', path)
    }
    .width(150)
    .padding({ top: 8, bottom: 8 })
    .backgroundColor(CARD)
    .borderRadius(14)
    .border({ width: 1, color: '#18000000' })
    .shadow({ radius: 20, color: '#1A000000', offsetY: 8 })
  }

  @Builder
  private ProjectCreateMenuItem(label: string, agentType: string, path: string) {
    Row() {
      Text(label)
        .fontSize(15)
        .fontColor(INK)
        .fontWeight(FontWeight.Medium)
    }
    .width('100%')
    .height(42)
    .padding({ left: 18, right: 18 })
    .onClick(() => {
      this.createMenuPath = '';
      this.onCreateInWorkspace(path, agentType);
    })
  }

  @Builder
  private ChatSection() {
    Column() {
      Row() {
        Row({ space: 8 }) {
          Text(RemoteI18n.t('remote.chats'))
            .fontSize(14)
            .fontWeight(FontWeight.Medium)
            .fontColor(MUTED)
          if (this.chatsCollapsed) {
            SymbolGlyph($r('sys.symbol.chevron_right'))
              .fontSize(14)
              .fontColor([MUTED])
              .width(18)
              .height(18)
          } else {
            SymbolGlyph($r('sys.symbol.chevron_down'))
              .fontSize(14)
              .fontColor([MUTED])
              .width(18)
              .height(18)
          }
        }
        .onClick(() => {
          this.chatsCollapsed = !this.chatsCollapsed;
        })
        Blank()
        Stack({ alignContent: Alignment.Center }) {
          SymbolGlyph($r('sys.symbol.square_and_pencil'))
            .fontSize(18)
            .fontColor([MUTED])
            .width(22)
            .height(22)
            .opacity(0.52)
        }
        .width(40)
        .height(40)
        .accessibilityText(RemoteI18n.t('remote.newChat'))
        .onClick(() => {
          this.onCreateAssistantSession();
        })
      }
      .width('100%')
      .height(44)
      .alignItems(VerticalAlign.Center)
      if (this.visibleChatSessions().length === 0) {
        this.EmptySessions()
      } else if (!this.chatsCollapsed) {
        ForEach(this.visibleChatSessions().slice(0, this.chatVisibleCount), (item: RemoteSession) => {
          this.SessionRow(item)
        })
        if (this.chatVisibleCount < this.visibleChatSessions().length) {
          Text(RemoteI18n.f('remote.sessions.showMore', String(this.nextChatBatchSize())))
            .fontSize(13)
            .fontColor(MUTED)
            .height(40)
            .width('100%')
            .textAlign(TextAlign.Center)
            .onClick(() => {
              this.chatVisibleCount += 3;
            })
        }
      }
    }
    .width('100%')
    .margin({ top: 8, bottom: 16 })
  }

  @Builder
  private projectPreview(path: string) {
    ForEach(this.visibleProjectSessions(path), (item: RemoteSession) => {
      this.SessionRow(item, true)
    })
    if (this.projectVisibleCount(path) < this.projectSessions(path).length) {
      Text(RemoteI18n.f('remote.sessions.showMore', String(this.nextProjectBatchSize(path))))
        .fontSize(15)
        .fontColor(MUTED)
        .height(44)
        .width('100%')
        .textAlign(TextAlign.Center)
        .onClick(() => {
          this.projectVisibleSteps = this.projectVisibleSteps.concat([path]);
        })
    }
  }

  private projectEntries(): RecentWorkspaceEntry[] {
    const entries: RecentWorkspaceEntry[] = [];
    if ((this.workspacePath.length > 0 || this.workspaceName.length > 0) && !this.isAssistantWorkspace(this.workspacePath)) {
      entries.push({ path: this.workspacePath, name: this.workspaceName, lastOpened: '', workspaceKind: 'normal' });
    }
    this.recentWorkspaces.forEach((item: RecentWorkspaceEntry) => {
      if (item.path.length > 0 && !this.isAssistantWorkspace(item.path) &&
        !entries.some((entry: RecentWorkspaceEntry) => entry.path === item.path)) {
        entries.push(item);
      }
    });
    if (!this.hasActiveListFilter()) {
      return entries;
    }
    return entries.filter((item: RecentWorkspaceEntry) => this.projectSessions(item.path).length > 0);
  }

  private visibleProjectEntries(): RecentWorkspaceEntry[] {
    return this.projectEntries().slice(0, this.visibleProjectCount);
  }

  private nextProjectEntryBatchSize(): number {
    return Math.min(3, this.projectEntries().length - this.visibleProjectCount);
  }

  private basename(path: string): string {
    const parts = path.split('/');
    return parts.length > 0 ? parts[parts.length - 1] : path;
  }

  private visibleChatSessions(): RemoteSession[] {
    return this.filteredSessions().filter((item: RemoteSession) => this.isAssistantSession(item));
  }

  private sessionsByTime(): RemoteSession[] {
    return this.filteredSessions().slice().sort((left: RemoteSession, right: RemoteSession) => {
      return this.sessionTimestamp(right) - this.sessionTimestamp(left);
    });
  }

  private sessionsForTimeBucket(bucket: string): RemoteSession[] {
    return this.sessionsByTime().filter((item: RemoteSession) => {
      return this.timeBucket(item) === bucket;
    });
  }

  private timeBucket(item: RemoteSession): string {
    const timestamp = this.sessionTimestamp(item);
    if (timestamp <= 0) {
      return 'earlier';
    }
    const sessionDate = new Date(timestamp);
    const now = new Date();
    if (this.sameCalendarDay(sessionDate, now)) {
      return 'today';
    }
    const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
    return this.sameCalendarDay(sessionDate, yesterday) ? 'yesterday' : 'earlier';
  }

  private sessionTimestamp(item: RemoteSession): number {
    const updated = TimeFormat.timestampMs(item.updatedAt || '');
    if (!Number.isNaN(updated) && updated > 0) {
      return updated;
    }
    const created = TimeFormat.timestampMs(item.createdAt || '');
    return !Number.isNaN(created) && created > 0 ? created : 0;
  }

  private sameCalendarDay(left: Date, right: Date): boolean {
    return left.getFullYear() === right.getFullYear() &&
      left.getMonth() === right.getMonth() &&
      left.getDate() === right.getDate();
  }

  private projectSessions(path: string): RemoteSession[] {
    return this.filteredSessions().filter((item: RemoteSession) => {
      return !this.isAssistantSession(item) && ConversationSessionFilterPolicy.workspacePathsEqual(
        item.workspacePath || this.workspacePath,
        path
      );
    });
  }

  private filteredSessions(): RemoteSession[] {
    return this.sessions.filter((item: RemoteSession) => {
      return ConversationSessionFilterPolicy.matches(
        item,
        this.query,
        this.workspacePath,
        this.workspaceFilter,
        this.agentFilter,
        this.statusFilter,
        this.isAssistantSession(item)
      );
    });
  }

  private hasActiveListFilter(): boolean {
    return this.query.trim().length > 0 || this.workspaceFilter.length > 0 ||
      this.agentFilter.length > 0 || this.statusFilter.length > 0;
  }

  private visibleProjectSessions(path: string): RemoteSession[] {
    return this.projectSessions(path).slice(0, this.projectVisibleCount(path));
  }

  private projectVisibleCount(path: string): number {
    const additionalSteps = this.projectVisibleSteps.filter((item: string) => item === path).length;
    return 3 + additionalSteps * 3;
  }

  private nextProjectBatchSize(path: string): number {
    return Math.min(3, this.projectSessions(path).length - this.projectVisibleCount(path));
  }

  private nextChatBatchSize(): number {
    return Math.min(3, this.visibleChatSessions().length - this.chatVisibleCount);
  }

  private isAssistantSession(item: RemoteSession): boolean {
    const agentType = (item.agentType || '').toLowerCase();
    return agentType === 'claw' || agentType === 'assistant' || agentType === 'chat' ||
      this.isAssistantWorkspace(item.workspacePath || '');
  }

  private isAssistantWorkspace(path: string): boolean {
    if (path.length === 0) {
      return this.workspaceKind.toLowerCase() === 'assistant';
    }
    if (path === this.workspacePath && this.workspaceKind.toLowerCase() === 'assistant') {
      return true;
    }
    return this.recentWorkspaces.some((item: RecentWorkspaceEntry) => {
      return item.path === path && item.workspaceKind.toLowerCase() === 'assistant';
    });
  }


  private isWorkspaceCollapsed(path: string): boolean {
    return this.collapsedWorkspacePaths.indexOf(path) >= 0;
  }

  private toggleWorkspace(path: string): void {
    if (this.isWorkspaceCollapsed(path)) {
      this.collapsedWorkspacePaths = this.collapsedWorkspacePaths.filter((item: string) => item !== path);
    } else {
      this.collapsedWorkspacePaths = this.collapsedWorkspacePaths.concat([path]);
    }
  }

  @Builder
  private SessionRow(item: RemoteSession, nested: boolean = false) {
    Row({ space: 8 }) {
      Column({ space: 2 }) {
        Text(item.title || RemoteI18n.t('sidebar.untitled'))
          .width('100%')
          .fontSize(15)
          .fontWeight(this.selectedSessionId === item.id ? FontWeight.Medium : FontWeight.Regular)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        if (this.metadataText(item).length > 0) {
          Text(this.metadataText(item))
            .width('100%')
            .fontSize(11)
            .fontColor(MUTED)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      this.SessionMoreButton(item)
    }
    .width('100%')
    .height(this.metadataText(item).length > 0 ? 56 : 46)
    .padding({ left: nested ? 0 : 10, right: 4 })
    .alignItems(VerticalAlign.Center)
    .backgroundColor(this.selectedSessionId === item.id ? SOFT : '#00000000')
    .borderRadius(10)
    .onClick(() => {
      this.onOpenSession(item);
    })
    .gesture(LongPressGesture({ repeat: false }).onAction(() => this.openSessionActions(item)))
    .bindPopup(this.actionPresentation === SessionActionPresentation.Popover &&
      this.activeActionSessionId === item.id, {
      builder: () => { this.SessionActionPopover() },
      placement: Placement.Right,
      popupColor: '#00000000',
      enableArrow: false,
      autoCancel: true,
      mask: false,
      targetSpace: 6,
      onStateChange: (event) => {
        if (!event.isVisible) {
          this.closeSessionActions();
        }
      }
    })
  }

  @Builder
  private SessionMoreButton(item: RemoteSession) {
    Stack({ alignContent: Alignment.Center }) {
      Row({ space: 3 }) {
        Text('').width(3.5).height(3.5).backgroundColor(MUTED).borderRadius(2)
        Text('').width(3.5).height(3.5).backgroundColor(MUTED).borderRadius(2)
        Text('').width(3.5).height(3.5).backgroundColor(MUTED).borderRadius(2)
      }
      .height(8)
      .alignItems(VerticalAlign.Center)
    }
    .width(34)
    .height(40)
    .opacity(0.62)
    .accessibilityText(RemoteI18n.t('session.actions'))
    .onClick(() => this.openSessionActions(item))
  }

  @Builder
  private SessionActionSheet() {
    this.SessionActionContent(SessionActionPresentation.BottomSheet)
  }

  @Builder
  private SessionActionPopover() {
    this.SessionActionContent(SessionActionPresentation.Popover)
  }

  @Builder
  private SessionActionContent(presentation: SessionActionPresentation) {
    SessionActionSurface({
      presentation,
      sessionTitle: this.actionSessionTitle(),
      canViewDetails: this.actionCapabilities().canViewDetails,
      canDelete: this.actionCapabilities().canDelete,
      onViewDetails: () => this.openActionSessionDetails(),
      onDelete: () => this.deleteActionSession(),
      onClose: () => this.closeSessionActions()
    })
  }

  @Builder
  private SessionDetailsSheet() {
    SessionDetailsView({
      session: this.detailsSession(),
      onClose: () => this.closeSessionDetails()
    })
  }

  @Builder
  private EmptySessions() {
    Column({ space: 8 }) {
      Text(this.isBusy ? RemoteI18n.t('home.emptyLoadingTitle') : RemoteI18n.t('remote.emptyTitle'))
        .fontSize(17)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Text(this.isBusy ? RemoteI18n.t('home.emptyLoadingText') : RemoteI18n.t('remote.emptyText'))
        .fontSize(14)
        .lineHeight(21)
        .fontColor(MUTED)
        .textAlign(TextAlign.Center)
    }
    .width('100%')
    .layoutWeight(1)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .padding({ left: 36, right: 36, bottom: 80 })
  }

  @Builder
  private FilteredEmptySessions() {
    Column({ space: 8 }) {
      Text(this.isBusy ? RemoteI18n.t('home.emptyLoadingTitle') : RemoteI18n.t('remote.emptyTitle'))
        .fontSize(17)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Text(this.isBusy ? RemoteI18n.t('home.emptyLoadingText') : RemoteI18n.t('remote.emptyText'))
        .fontSize(14)
        .lineHeight(21)
        .fontColor(MUTED)
        .textAlign(TextAlign.Center)
    }
    .width('100%')
    .alignItems(HorizontalAlign.Center)
    .padding({ left: 24, right: 24, top: 24, bottom: 16 })
  }

  private openSessionActions(item: RemoteSession): void {
    if (this.isBusy || item.id.length === 0) {
      return;
    }
    this.activeActionSessionId = item.id;
    if (this.actionPresentation === SessionActionPresentation.BottomSheet) {
      this.showSessionActionSheet = true;
    }
  }

  private closeSessionActions(): void {
    this.showSessionActionSheet = false;
    this.activeActionSessionId = '';
  }

  private actionSession(): RemoteSession | undefined {
    return this.sessions.find((item: RemoteSession) => item.id === this.activeActionSessionId);
  }

  private actionSessionTitle(): string {
    const session = this.actionSession();
    return session ? session.title : '';
  }

  private metadataText(item: RemoteSession): string {
    const values: string[] = [];
    if (this.showWorkspaceMetadata) {
      const workspace = item.workspaceName || item.workspacePath || '';
      if (workspace.length > 0) {
        values.push(workspace);
      }
    }
    if (this.showUpdatedMetadata && !Number.isNaN(TimeFormat.timestampMs(item.updatedAt))) {
      values.push(TimeFormat.relative(item.updatedAt));
    }
    if (this.showStatusMetadata && item.status.length > 0) {
      values.push(item.status === 'archived' ? RemoteI18n.t('sidebar.archived') : item.status);
    }
    return values.join(' · ');
  }

  private actionCapabilities(): SessionActionCapabilities {
    const session = this.actionSession();
    return SessionActionPolicy.resolve(
      SessionActionScope.Remote,
      session ? session.agentType : '',
      this.isBusy || session === undefined
    );
  }

  private deleteActionSession(): void {
    const session = this.actionSession();
    if (session) {
      this.onDeleteSession(session);
    }
  }

  private openActionSessionDetails(): void {
    const session = this.actionSession();
    if (session) {
      this.detailsSessionId = session.id;
      this.showSessionDetails = true;
    }
  }

  private closeSessionDetails(): void {
    this.showSessionDetails = false;
    this.detailsSessionId = '';
  }

  private detailsSession(): RemoteSession {
    const session = this.sessions.find((item: RemoteSession) => item.id === this.detailsSessionId);
    return session || {
      id: '', title: '', agentType: '', status: '', updatedAt: '', createdAt: '', messageCount: 0
    };
  }

  private sessionActionSheetOptions(): SheetOptions {
    return {
      height: 300,
      backgroundColor: '#00000000',
      maskColor: '#44000000',
      showClose: false,
      dragBar: false
    };
  }

  private sessionDetailsSheetOptions(): SheetOptions {
    if (this.actionPresentation === SessionActionPresentation.BottomSheet) {
      return {
        height: SheetSize.LARGE,
        backgroundColor: '#00000000',
        maskColor: '#44000000',
        showClose: false,
        dragBar: false
      };
    }
    return {
      height: 560,
      width: 560,
      preferType: SheetType.CENTER,
      backgroundColor: '#00000000',
      maskColor: '#44000000',
      showClose: false,
      dragBar: false
    };
  }
}
