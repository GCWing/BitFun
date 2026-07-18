import { AssistantEntry, RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ConnectionStatusPresenter } from '../../services/ConnectionStatusPresenter';
import { TimeFormat } from '../../services/TimeFormat';
import { ACCENT, CARD, GREEN, INK, LINE, MUTED, RED, SOFT, SUBTLE } from './Theme';

@Component
export struct HomeView {
  @Prop desktopName: string = 'MacBook Pro';
  @Prop workspaceName: string = 'BitFun';
  @Prop workspacePath: string = '';
  @Prop workspaceBranch: string = '';
  @Prop workspaceKind: string = 'normal';
  @Prop assistantId: string = '';
  @Prop authenticatedUserId: string = '';
  @Prop statusText: string = RemoteI18n.t('connection.connected');
  @Prop connectionState: string = 'connected';
  @Prop sessions: RemoteSession[] = [];
  @Link sessionQuery: string;
  @Prop sessionFilter: string = 'all';
  @Prop hasMoreSessions: boolean = false;
  @Prop isBusy: boolean = false;
  @Prop recentWorkspaces: RecentWorkspaceEntry[] = [];
  @Prop assistants: AssistantEntry[] = [];
  @Prop showWorkspacePicker: boolean = false;
  @Prop showAssistantPicker: boolean = false;
  onBack: () => void = () => {};
  onConnectWorkspace: () => void = () => {};
  onRefresh: () => void = () => {};
  onShowWorkspaces: () => void = () => {};
  onShowAssistants: () => void = () => {};
  onSelectWorkspace: (path: string) => void = (_path: string) => {};
  onSelectAssistant: (path: string) => void = (_path: string) => {};
  onCancelWorkspacePicker: () => void = () => {};
  onCancelAssistantPicker: () => void = () => {};
  onSearchSessions: () => void = () => {};
  onSetSessionFilter: (filter: string) => void = (_filter: string) => {};
  onLoadMoreSessions: () => void = () => {};
  onReconnect: () => void = () => {};
  onDisconnect: () => void = () => {};
  onClearPairing: () => void = () => {};
  onCreate: (agentType: string) => void = (_agentType: string) => {};
  onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  onDeleteSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @State showAllSessions: boolean = false;

  build() {
    Scroll() {
      Column({ space: 16 }) {
        this.Header()
        this.Hero()
        if (this.isConnected()) {
          this.ConnectedPanel()
          if (this.showWorkspacePicker) {
            this.WorkspacePicker()
          }
          this.RecentSessions()
        } else {
          this.ConnectEntry()
          this.RecentWorkspacesPreview()
          this.RecentSessions()
        }
      }
      .width('100%')
      .padding({ left: 24, right: 24, top: 34, bottom: 30 })
    }
    .scrollBar(BarState.Off)
  }

  @Builder
  Header() {
    Row() {
      Text('‹')
        .width(42)
        .height(42)
        .fontSize(34)
        .fontColor(INK)
        .textAlign(TextAlign.Center)
        .onClick(() => {
          this.onBack();
        })
      Text(RemoteI18n.t('code.title'))
        .fontSize(22)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
        .layoutWeight(1)
        .textAlign(TextAlign.Center)
      Text('+')
        .width(42)
        .height(42)
        .fontSize(28)
        .fontColor(this.isConnected() ? INK : SUBTLE)
        .textAlign(TextAlign.Center)
        .onClick(() => {
          if (this.isConnected()) {
            this.onCreate('code');
          }
        })
    }
    .width('100%')
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  Hero() {
    Column({ space: 8 }) {
      Text(RemoteI18n.t('code.heroTitle'))
        .fontSize(25)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
        .width('100%')
      Text(RemoteI18n.t('code.heroBody'))
        .fontSize(14)
        .lineHeight(21)
        .fontColor(MUTED)
        .width('100%')
    }
    .width('100%')
    .padding({ top: 12, bottom: 4 })
  }

  @Builder
  ConnectedPanel() {
    Column({ space: 14 }) {
      Row({ space: 10 }) {
        Text('●')
          .fontSize(13)
          .fontColor(this.connectionColor())
        Text(ConnectionStatusPresenter.label(this.connectionState))
          .fontSize(15)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Text(this.desktopName)
          .fontSize(15)
          .fontColor(MUTED)
          .layoutWeight(1)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .width('100%')

      Column({ space: 5 }) {
        Text(RemoteI18n.t('home.currentWorkspace'))
          .fontSize(13)
          .fontColor(MUTED)
          .width('100%')
        Text(this.workspaceTitle())
          .fontSize(24)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .width('100%')
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        if (this.workspaceBranch.length > 0) {
          Text(this.workspaceBranch)
            .fontSize(13)
            .fontColor(SUBTLE)
            .width('100%')
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
      }
      .width('100%')

      Button(RemoteI18n.t('code.newTask'))
        .width('100%')
        .height(52)
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(CARD)
        .backgroundColor(ACCENT)
        .borderRadius(14)
        .enabled(!this.isBusy)
        .onClick(() => {
          this.onCreate('code');
        })

      Row({ space: 10 }) {
        this.SecondaryButton(RemoteI18n.t('code.switchWorkspace'), () => {
          this.onShowWorkspaces();
        })
        this.SecondaryButton(RemoteI18n.t('code.disconnect'), () => {
          this.onDisconnect();
        })
      }
      .width('100%')
    }
    .width('100%')
    .padding({ left: 18, right: 18, top: 18, bottom: 18 })
    .backgroundColor(CARD)
    .borderRadius(16)
    .border({ width: 1, color: LINE })
  }

  @Builder
  ConnectEntry() {
    Row({ space: 14 }) {
      this.DesktopGlyph()
      Column({ space: 4 }) {
        Text(RemoteI18n.t('code.connectWorkspace'))
          .fontSize(17)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .width('100%')
        Text(RemoteI18n.t('code.connectDesc'))
          .fontSize(13)
          .lineHeight(18)
          .fontColor(MUTED)
          .width('100%')
          .maxLines(2)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      Text('›')
        .fontSize(24)
        .fontColor(MUTED)
    }
    .width('100%')
    .padding({ left: 18, right: 18, top: 18, bottom: 18 })
    .backgroundColor(CARD)
    .borderRadius(16)
    .border({ width: 1, color: LINE })
    .onClick(() => {
      this.onConnectWorkspace();
    })
  }

  @Builder
  RecentWorkspacesPreview() {
    Column({ space: 10 }) {
      Text(RemoteI18n.t('home.recentWorkspaces'))
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .width('100%')
      if (this.recentWorkspaces.length === 0) {
        Text(RemoteI18n.t('code.noWorkspacePreview'))
          .fontSize(13)
          .lineHeight(19)
          .fontColor(MUTED)
          .width('100%')
      } else {
        ForEach(this.previewWorkspaces(), (item: RecentWorkspaceEntry) => {
          this.WorkspacePreviewRow(item)
        }, (item: RecentWorkspaceEntry) => item.path)
      }
    }
    .width('100%')
    .padding({ left: 18, right: 18, top: 18, bottom: 16 })
    .backgroundColor(CARD)
    .borderRadius(16)
    .border({ width: 1, color: LINE })
  }

  @Builder
  WorkspacePicker() {
    Column({ space: 0 }) {
      Row() {
        Text(RemoteI18n.t('home.recentWorkspaces'))
          .fontSize(15)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        Text(RemoteI18n.t('common.close'))
          .fontSize(13)
          .fontColor(MUTED)
          .onClick(() => {
            this.onCancelWorkspacePicker();
          })
      }
      .width('100%')
      .margin({ bottom: 10 })

      if (this.recentWorkspaces.length === 0) {
        Text(this.isBusy ? RemoteI18n.t('common.loading') : RemoteI18n.t('home.noRecentWorkspaces'))
          .fontSize(13)
          .fontColor(MUTED)
          .width('100%')
          .padding({ top: 12, bottom: 12 })
      } else {
        ForEach(this.recentWorkspaces, (item: RecentWorkspaceEntry) => {
          this.WorkspaceRow(item)
        }, (item: RecentWorkspaceEntry) => item.path)
      }
    }
    .width('100%')
    .padding({ left: 18, right: 18, top: 18, bottom: 14 })
    .backgroundColor(CARD)
    .borderRadius(16)
    .border({ width: 1, color: LINE })
  }

  @Builder
  WorkspaceRow(item: RecentWorkspaceEntry) {
    Row({ space: 12 }) {
      Column({ space: 4 }) {
        Text(item.name || this.basename(item.path))
          .fontSize(14)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(item.path)
          .fontSize(12)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      Text(item.path === this.workspacePath ? RemoteI18n.t('common.current') : RemoteI18n.t('common.open'))
        .fontSize(12)
        .fontColor(item.path === this.workspacePath ? MUTED : INK)
    }
    .width('100%')
    .padding({ top: 12, bottom: 12 })
    .border({ width: { bottom: 1 }, color: LINE })
    .onClick(() => {
      if (item.path !== this.workspacePath) {
        this.showAllSessions = false;
        this.onSelectWorkspace(item.path);
      }
    })
  }

  @Builder
  WorkspacePreviewRow(item: RecentWorkspaceEntry) {
    Row() {
      Text(item.name || this.basename(item.path))
        .fontSize(15)
        .fontColor(INK)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
      Text('›')
        .fontSize(20)
        .fontColor(SUBTLE)
    }
    .width('100%')
    .height(42)
    .border({ width: { bottom: 1 }, color: LINE })
    .onClick(() => {
      this.onConnectWorkspace();
    })
  }

  @Builder
  RecentSessions() {
    Column({ space: 0 }) {
      Row() {
        Text(RemoteI18n.t('code.recentSessions'))
          .fontSize(16)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        if (this.isConnected()) {
          Text(RemoteI18n.t('common.refresh'))
            .fontSize(13)
            .fontColor(MUTED)
            .onClick(() => {
              this.onRefresh();
            })
        }
      }
      .width('100%')
      .margin({ bottom: 10 })

      if (this.codeSessions().length === 0) {
        this.SessionEmptyState()
      } else {
        List() {
          ForEach(this.visibleSessions(), (item: RemoteSession) => {
            ListItem() {
              this.SessionRowContent(item)
            }
            .height(68)
            .swipeAction(this.sessionSwipeAction(item))
          }, (item: RemoteSession) => item.id)
          if (this.shouldShowViewAll()) {
            ListItem() {
              this.ViewAllRow()
            }
            .height(48)
          }
        }
        .width('100%')
        .height(this.recentListHeight())
        .scrollBar(BarState.Off)
        .divider(null)
      }
    }
    .width('100%')
    .padding({ left: 18, right: 18, top: 18, bottom: 12 })
    .backgroundColor(CARD)
    .borderRadius(16)
    .border({ width: 1, color: LINE })
  }

  @Builder
  SessionRowContent(item: RemoteSession) {
    Row({ space: 12 }) {
      Column({ space: 4 }) {
        Text(item.title)
          .fontSize(16)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(`${this.displayAgent(item.agentType)} · ${TimeFormat.relative(item.updatedAt || item.createdAt)}`)
          .fontSize(12)
          .fontColor(MUTED)
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      Text('›')
        .fontSize(22)
        .fontColor(SUBTLE)
    }
    .width('100%')
    .height(68)
    .alignItems(VerticalAlign.Center)
    .backgroundColor(CARD)
    .border({ width: { bottom: 1 }, color: LINE })
    .onClick(() => {
      this.onOpenSession(item);
    })
  }

  @Builder
  ViewAllRow() {
    Row() {
      Text(this.isBusy ? RemoteI18n.t('common.loading') : RemoteI18n.t('home.viewAll'))
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Blank()
      Text(this.sessionCountText())
        .fontSize(12)
        .fontColor(SUBTLE)
      Text('›')
        .fontSize(20)
        .fontColor(SUBTLE)
        .margin({ left: 6 })
    }
    .width('100%')
    .height(48)
    .alignItems(VerticalAlign.Center)
    .backgroundColor(CARD)
    .onClick(() => {
      if (this.isBusy) {
        return;
      }
      this.showAllSessions = true;
      if (this.hasMoreSessions) {
        this.onLoadMoreSessions();
      }
    })
  }

  @Builder
  SessionEmptyState() {
    Row({ space: 12 }) {
      Text(this.isBusy ? '◌' : '∅')
        .width(38)
        .height(38)
        .fontSize(18)
        .fontColor(MUTED)
        .textAlign(TextAlign.Center)
        .backgroundColor(SOFT)
        .borderRadius(14)
      Column({ space: 4 }) {
        Text(this.isBusy ? RemoteI18n.t('home.emptyLoadingTitle') : RemoteI18n.t('code.emptySessionTitle'))
          .fontSize(14)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Text(this.isBusy ? RemoteI18n.t('home.emptyLoadingText') : RemoteI18n.t('code.emptySessionText'))
          .fontSize(12)
          .lineHeight(17)
          .fontColor(MUTED)
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%')
    .padding({ top: 14, bottom: 14 })
  }

  @Builder
  DeleteReveal(item: RemoteSession) {
    Text(RemoteI18n.t('home.deleteSession'))
      .fontSize(13)
      .fontColor(CARD)
      .textAlign(TextAlign.Center)
      .width(92)
      .height(68)
      .backgroundColor(RED)
      .onClick(() => {
        this.onDeleteSession(item);
      })
  }

  @Builder
  SecondaryButton(label: string, action: () => void) {
    Button(label)
      .layoutWeight(1)
      .height(44)
      .fontSize(14)
      .fontWeight(FontWeight.Medium)
      .fontColor(INK)
      .backgroundColor(SOFT)
      .borderRadius(12)
      .onClick(action)
  }

  @Builder
  DesktopGlyph() {
    Stack() {
      Text('')
        .width(30)
        .height(22)
        .borderRadius(4)
        .border({ width: 2, color: INK })
        .position({ x: 5, y: 4 })
      Text('')
        .width(10)
        .height(2)
        .backgroundColor(INK)
        .position({ x: 15, y: 29 })
      Text('')
        .width(22)
        .height(2)
        .backgroundColor(INK)
        .position({ x: 9, y: 34 })
    }
    .width(42)
    .height(42)
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

  private isConnected(): boolean {
    return this.connectionState === 'connected' || this.connectionState === 'reconnecting';
  }

  private workspaceTitle(): string {
    return this.workspaceName || 'BitFun';
  }

  private previewWorkspaces(): RecentWorkspaceEntry[] {
    return this.recentWorkspaces.slice(0, 3);
  }

  private codeSessions(): RemoteSession[] {
    return this.sessions.filter((item: RemoteSession) => (item.agentType || '').toLowerCase() !== 'cowork');
  }

  private visibleSessions(): RemoteSession[] {
    const items = this.codeSessions();
    if (this.showAllSessions) {
      return items;
    }
    return items.slice(0, this.collapsedSessionLimit());
  }

  private shouldShowViewAll(): boolean {
    return this.codeSessions().length > 0;
  }

  private recentListHeight(): number {
    const sessionHeight = this.visibleSessions().length * 68;
    return sessionHeight + (this.shouldShowViewAll() ? 48 : 0);
  }

  private collapsedSessionLimit(): number {
    return 3;
  }

  private basename(path: string): string {
    if (!path) {
      return 'Workspace';
    }
    const parts = path.replace(/\\/g, '/').split('/');
    return parts[parts.length - 1] || 'Workspace';
  }

  private connectionColor(): string {
    const tone = ConnectionStatusPresenter.tone(this.connectionState);
    if (tone === 'ok') {
      return GREEN;
    }
    if (tone === 'error') {
      return RED;
    }
    return MUTED;
  }

  private sessionCountText(): string {
    return RemoteI18n.f('home.sessionCount', `${this.codeSessions().length}`);
  }

  private displayAgent(agentType: string): string {
    const normalized = (agentType || '').toLowerCase();
    if (normalized === 'cowork') {
      return 'Cowork';
    }
    return 'Code';
  }
}
