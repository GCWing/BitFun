import { RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { TimeFormat } from '../../services/TimeFormat';
import { CARD, INK, MUTED, RED } from './Theme';

@ComponentV2
export struct RemoteSessionList {
  @Param sessions: RemoteSession[] = [];
  @Param query: string = '';
  @Param sortMode: string = 'project';
  @Param hasMoreSessions: boolean = false;
  @Param isBusy: boolean = false;
  @Param workspaceName: string = '';
  @Param workspacePath: string = '';
  @Param recentWorkspaces: RecentWorkspaceEntry[] = [];
  @Event onCreate: () => void = () => {};
  @Event onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onDeleteSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onLoadMore: () => void = () => {};
  @Event onCreateAssistantSession: () => void = () => {};
  @Event onCreateInWorkspace: (path: string) => void = (_path: string) => {};
  @Event onSelectWorkspace: (path: string) => void = (_path: string) => {};
  @Local collapsedWorkspacePaths: string[] = [];
  @Local projectVisibleSteps: string[] = [];
  @Local chatsCollapsed: boolean = false;
  @Local chatVisibleCount: number = 3;
  @Local todayCollapsed: boolean = false;
  @Local yesterdayCollapsed: boolean = false;
  @Local earlierCollapsed: boolean = false;

  build() {
    Column() {
      Scroll() {
        Column() {
          if (this.sortMode === 'time') {
            this.TimeSection()
          } else if (this.sortMode === 'chat') {
            this.ChatSection()
            this.ProjectSection()
          } else {
            this.ProjectSection()
            this.ChatSection()
          }
        }
        .width('100%')
      }
      .layoutWeight(1)
      .width('100%')
      .scrollable(ScrollDirection.Vertical)
      .scrollBar(BarState.Off)
    }
    .width('100%')
    .layoutWeight(1)
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
    .margin({ top: 14, bottom: 16 })
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
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
      if (collapsed) {
        Image($r('app.media.remote_ref_chevron_right'))
          .width(16)
          .height(16)
          .objectFit(ImageFit.Contain)
          .opacity(0.55)
      } else {
        Image($r('app.media.remote_ref_chevron_centered'))
          .width(16)
          .height(16)
          .objectFit(ImageFit.Contain)
          .opacity(0.55)
      }
    }
    .width('100%')
    .height(50)
    .alignItems(VerticalAlign.Center)
    .onClick(action)
  }

  @Builder
  private ProjectSection() {
    Column({ space: 4 }) {
      Text(RemoteI18n.t('sidebar.projects'))
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
        .width('100%')
        .margin({ bottom: 8 })
      ForEach(this.projectEntries(), (project: RecentWorkspaceEntry) => {
        Column({ space: 2 }) {
          Row({ space: 10 }) {
            Image($r('app.media.remote_ref_folder'))
              .width(30)
              .height(25)
              .objectFit(ImageFit.Contain)
            Text(project.name || this.basename(project.path))
              .fontSize(18)
              .fontColor(INK)
              .layoutWeight(1)
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
            if (this.isWorkspaceCollapsed(project.path)) {
              Image($r('app.media.remote_ref_chevron_right'))
                .width(18)
                .height(18)
                .objectFit(ImageFit.Contain)
            } else {
              Image($r('app.media.remote_ref_chevron_centered'))
                .width(18)
                .height(18)
                .objectFit(ImageFit.Contain)
            }
            Image($r('app.media.remote_ref_edit'))
              .width(25)
              .height(25)
              .objectFit(ImageFit.Contain)
              .opacity(0.52)
              .onClick(() => {
                this.onCreateInWorkspace(project.path);
              })
          }
          .width('100%')
          .height(50)
          .onClick(() => {
            this.toggleWorkspace(project.path);
          })
          if (!this.isWorkspaceCollapsed(project.path)) {
            this.projectPreview(project.path)
          }
        }
        .width('100%')
        .margin({ bottom: 8 })
      })
    }
    .width('100%')
    .margin({ top: 18 })
  }

  @Builder
  private ChatSection() {
    Column() {
      Row() {
        Row({ space: 8 }) {
          Text(RemoteI18n.t('remote.chats'))
            .fontSize(20)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
          if (this.chatsCollapsed) {
            Image($r('app.media.remote_ref_chevron_right'))
              .width(18)
              .height(18)
              .objectFit(ImageFit.Contain)
          } else {
            Image($r('app.media.remote_ref_chevron_centered'))
              .width(18)
              .height(18)
              .objectFit(ImageFit.Contain)
          }
        }
        .onClick(() => {
          this.chatsCollapsed = !this.chatsCollapsed;
        })
        Blank()
        Image($r('app.media.remote_ref_edit'))
          .width(26)
          .height(26)
          .objectFit(ImageFit.Contain)
          .opacity(0.52)
          .onClick(() => {
            this.onCreateAssistantSession();
          })
      }
      .width('100%')
      .height(58)
      .alignItems(VerticalAlign.Center)
      if (this.visibleChatSessions().length === 0) {
        this.EmptySessions()
      } else if (!this.chatsCollapsed) {
        ForEach(this.visibleChatSessions().slice(0, this.chatVisibleCount), (item: RemoteSession) => {
          this.SessionRow(item)
        })
        if (this.chatVisibleCount < this.visibleChatSessions().length) {
          Text(`再显示 ${this.nextChatBatchSize()} 条`)
            .fontSize(15)
            .fontColor(MUTED)
            .height(48)
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
      Text(item.title || RemoteI18n.t('sidebar.untitled'))
        .fontSize(16)
        .fontColor(INK)
        .width('100%')
        .height(58)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .padding({ left: 0 })
        .textAlign(TextAlign.Start)
        .onClick(() => {
          this.onOpenSession(item);
        })
    })
    if (this.projectVisibleCount(path) < this.projectSessions(path).length) {
      Text(`再显示 ${this.nextProjectBatchSize(path)} 条`)
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
    if (this.workspacePath.length > 0 || this.workspaceName.length > 0) {
      entries.push({ path: this.workspacePath, name: this.workspaceName, lastOpened: '', workspaceKind: 'normal' });
    }
    this.recentWorkspaces.forEach((item: RecentWorkspaceEntry) => {
      if (item.path.length > 0 && !entries.some((entry: RecentWorkspaceEntry) => entry.path === item.path)) {
        entries.push(item);
      }
    });
    return entries;
  }

  private basename(path: string): string {
    const parts = path.split('/');
    return parts.length > 0 ? parts[parts.length - 1] : path;
  }

  private visibleChatSessions(): RemoteSession[] {
    const query = this.query.trim().toLowerCase();
    return this.sessions.filter((item: RemoteSession) => {
      if (!this.isAssistantSession(item) || item.status === 'archived') {
        return false;
      }
      return query.length === 0 || item.title.toLowerCase().indexOf(query) >= 0;
    });
  }

  private sessionsByTime(): RemoteSession[] {
    const query = this.query.trim().toLowerCase();
    return this.sessions.filter((item: RemoteSession) => {
      if (item.status === 'archived') {
        return false;
      }
      return query.length === 0 || item.title.toLowerCase().indexOf(query) >= 0;
    }).slice().sort((left: RemoteSession, right: RemoteSession) => {
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
    return this.sessions.filter((item: RemoteSession) => {
      return !this.isAssistantSession(item) && (item.workspacePath || this.workspacePath) === path;
    });
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
    return agentType === 'claw' || agentType === 'assistant' || agentType === 'chat';
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
  private SessionRow(item: RemoteSession) {
    Row() {
      Text(item.title || RemoteI18n.t('sidebar.untitled'))
        .fontSize(18)
        .fontWeight(FontWeight.Regular)
        .fontColor(INK)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
      Text('›')
        .fontSize(22)
        .fontColor('#00000000')
    }
    .width('100%')
    .height(58)
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      this.onOpenSession(item);
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
  private DeleteReveal(item: RemoteSession) {
    Text(RemoteI18n.t('home.deleteSession'))
      .fontSize(13)
      .fontColor(CARD)
      .textAlign(TextAlign.Center)
      .width(92)
      .height(58)
      .backgroundColor(RED)
      .onClick(() => {
        this.onDeleteSession(item);
      })
  }

  private sessionSwipeAction(item: RemoteSession): SwipeActionOptions {
    if (this.isBusy || item.id.length === 0) {
      return {};
    }
    return {
      end: {
        builder: () => {
          this.DeleteReveal(item);
        },
        actionAreaDistance: 92,
        onAction: () => {
          this.onDeleteSession(item);
        }
      },
      edgeEffect: SwipeEdgeEffect.None
    };
  }
}
