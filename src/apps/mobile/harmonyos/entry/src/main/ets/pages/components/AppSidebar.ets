import { RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, GREEN, INK, LINE, MUTED, PAGE_BG, SOFT, SUBTLE } from './Theme';
import { ConversationSource } from '../navigation/AppRouteContract';
import { ConversationSourceSwitcher } from './ConversationSourceSwitcher';
import { SidebarToggleButton } from './SidebarToggleButton';
import { SessionActionPresentation, SessionActionSurface } from './SessionActionSurface';
import { SessionActionCapabilities, SessionActionPolicy, SessionActionScope } from '../policy/SessionActionPolicy';
import { SessionDetailsView } from './SessionDetailsView';
import { SidebarGlyph } from './SidebarGlyphs';

@ComponentV2
export struct AppSidebar {
  @Param sessions: RemoteSession[] = [];
  @Param pinnedSessionId: string = '';
  @Param selectedSessionId: string = '';
  @Param connectionState: string = 'idle';
  @Param activeSection: string = 'chat';
  @Param accountUserId: string = '';
  @Param showConversationSourceSwitcher: boolean = false;
  @Param showCollapseButton: boolean = false;
  @Param showViewSettingsButton: boolean = false;
  @Param showCustomContent: boolean = false;
  @Param conversationSource: ConversationSource = ConversationSource.General;
  @Event onClose: () => void = () => {};
  @Event onNewChat: () => void = () => {};
  @Event onEnterCode: () => void = () => {};
  @Event onConversationSource: (source: ConversationSource) => void = (_source: ConversationSource) => {};
  @Event onCollapse: () => void = () => {};
  @Event onOpenViewSettings: () => void = () => {};
  @Event onSearchQueryChange: (query: string) => void = (_query: string) => {};
  @Event onOpenSettings: () => void = () => {};
  @Event onOpenAccount: () => void = () => {};
  @Event onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onArchiveSession: (session: RemoteSession, archived: boolean) => void =
    (_session: RemoteSession, _archived: boolean) => {};
  @Event onExportSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onDeleteSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Local activeActionSessionId: string = '';
  @Local showSessionActionSheet: boolean = false;
  @Local detailsSessionId: string = '';
  @Local showSessionDetails: boolean = false;
  @Local showSearch: boolean = false;
  @Local sessionSearchQuery: string = '';
  @Local archivedSessionsExpanded: boolean = false;
  /**
   * Session content for the current conversation source. The shell around it
   * (header, source switcher, content origin, footer) stays identical for every
   * source so switching Local/Remote never moves shared chrome.
   */
  @BuilderParam contentSlot: () => void = this.LocalSessionContent;

  build() {
    this.SidebarContent()
  }

  @Builder
  private SidebarContent() {
    Column() {
      if (this.isAccountAuthenticated()) {
        this.AuthenticatedHeader()
      } else {
        this.SignedOutHeader()
      }

      Column({ space: 0 }) {
        if (this.showConversationSourceSwitcher) {
          ConversationSourceSwitcher({
            activeSource: this.conversationSource,
            onSelectSource: this.onConversationSource
          })
        } else {
          this.NavRow(RemoteI18n.t('sidebar.code'), this.activeSection === 'remote', () => {
            this.onEnterCode();
          })
        }
      }
      .width('100%')
      .margin({ top: 18 })

      Stack({ alignContent: Alignment.Bottom }) {
        Column() {
          if (this.showCustomContent) {
            this.contentSlot()
          } else {
            this.LocalSessionContent()
          }
        }
        .width('100%')
        .height('100%')
        .alignItems(HorizontalAlign.Start)

        if (this.isAccountAuthenticated()) {
          this.AuthenticatedFooter()
        } else {
          this.SignedOutFooter()
        }
      }
      .width('100%')
      .layoutWeight(1)
    }
    .width('100%')
    .height('100%')
    .padding({ left: 20, right: 20, top: 4, bottom: 16 })
    .backgroundColor(PAGE_BG)
    .bindSheet($$this.showSessionActionSheet, this.SessionActionSheet(), this.sessionActionSheetOptions())
    .bindSheet($$this.showSessionDetails, this.SessionDetailsSheet(), this.sessionDetailsSheetOptions())
  }

  @Builder
  LocalSessionContent() {
    Column() {
      if (this.visiblePinnedSessions().length > 0) {
        Text('置顶')
          .fontSize(14).fontWeight(FontWeight.Medium).fontColor(MUTED)
          .width('100%').margin({ top: 16, bottom: 6 })
        ForEach(this.visiblePinnedSessions(), (session: RemoteSession) => {
          this.PinnedRow(session)
        }, (session: RemoteSession) => session.id)
      }

      Text(RemoteI18n.t('sidebar.recent'))
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
        .width('100%')
        .margin({ top: 16, bottom: 6 })

      List({ space: 2 }) {
        if (this.visibleRecentSessions().length === 0) {
          ListItem() {
            this.EmptyRecent()
          }
        }
        Repeat<RemoteSession>(this.visibleRecentSessions())
          .each((obj: RepeatItem<RemoteSession>) => {
            ListItem() {
              this.RecentRow(obj)
            }
          })
          .key((item: RemoteSession) => item.id)
          .virtualScroll({ totalCount: this.visibleRecentSessions().length })
        if (this.archivedSessionCount() > 0) {
          ListItem() {
            this.ArchivedDisclosureRow()
          }
        }
        if (this.archivedSessionsExpanded) {
          Repeat<RemoteSession>(this.visibleArchivedSessions())
            .each((obj: RepeatItem<RemoteSession>) => {
              ListItem() {
                this.RecentRow(obj)
              }
            })
            .key((item: RemoteSession) => item.id)
            .virtualScroll({ totalCount: this.visibleArchivedSessions().length })
        }
      }
      .width('100%')
      .layoutWeight(1)
      .margin({ left: 0 })
      .padding({ bottom: 84 })
      .scrollBar(BarState.Off)
      .divider(null)
    }
    .width('100%')
    .height('100%')
    .alignItems(HorizontalAlign.Start)
  }

  @Builder
  private AuthenticatedHeader() {
    Row() {
      Text('BitFun')
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
      Blank()
      // Right-aligned so the search and collapse controls keep the same
      // position whether or not the source-specific view-settings entry shows.
      Row({ space: 6 }) {
        if (this.showViewSettingsButton) {
          Stack({ alignContent: Alignment.Center }) {
            SidebarGlyph({ kind: 'session_more' })
          }
          .width(38)
          .height(38)
          .backgroundColor(CARD)
          .border({ width: 1, color: LINE })
          .borderRadius(19)
          .shadow({ radius: 14, color: '#16000000', offsetY: 6 })
          .accessibilityText(RemoteI18n.t('remote.actions'))
          .onClick(() => {
            this.onOpenViewSettings();
          })
        }

        Stack({ alignContent: Alignment.Center }) {
          SidebarGlyph({ kind: 'search' })
        }
        .width(38)
        .height(38)
        .backgroundColor(CARD)
        .border({ width: 1, color: LINE })
        .borderRadius(19)
        .shadow({ radius: 14, color: '#16000000', offsetY: 6 })
        .accessibilityText(RemoteI18n.t('common.search'))
        .onClick(() => {
          this.showSearch = !this.showSearch;
          if (!this.showSearch) {
            this.updateSearchQuery('');
          }
        })

        if (this.showCollapseButton) {
          SidebarToggleButton({
            controlSize: 38,
            onToggle: this.onCollapse
          })
        }
      }
    }
    .width('100%')
    .height(50)
    .alignItems(VerticalAlign.Center)

    if (this.showSearch) {
      TextInput({ placeholder: RemoteI18n.t('sidebar.searchPlaceholder'), text: this.sessionSearchQuery })
        .width('100%')
        .height(42)
        .fontSize(14)
        .fontColor(INK)
        .placeholderColor(SUBTLE)
        .padding({ left: 14, right: 14 })
        .backgroundColor(SOFT)
        .borderRadius(8)
        .margin({ top: 12 })
        .onChange((value: string) => {
          this.updateSearchQuery(value);
        })
    }
  }

  private updateSearchQuery(value: string): void {
    this.sessionSearchQuery = value;
    this.onSearchQueryChange(value);
  }

  @Builder
  private SignedOutHeader() {
    Row() {
      Row({ space: 14 }) {
        SymbolGlyph($r('sys.symbol.square_and_pencil'))
          .fontSize(23)
          .fontColor([INK])
          .width(24)
          .height(24)
        Text(RemoteI18n.t('sidebar.signedOutNewChat'))
          .fontSize(16)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
      }
      .layoutWeight(1)
      .height(50)
      .alignItems(VerticalAlign.Center)
      .onClick(() => {
        this.onNewChat();
      })

      if (this.showCollapseButton) {
        SidebarToggleButton({
          controlSize: 38,
          onToggle: this.onCollapse
        })
      }
    }
    .width('100%')
    .height(50)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private AuthenticatedFooter() {
    Row() {
      Row({ space: 9 }) {
        SidebarGlyph({ kind: 'edit' })
        Text(RemoteI18n.t('sidebar.newChat'))
          .fontSize(15)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
      }
      .width(116)
      .height(46)
      .justifyContent(FlexAlign.Center)
      .alignItems(VerticalAlign.Center)
      .backgroundColor(CARD)
      .border({ width: 1, color: LINE })
      .borderRadius(23)
      .shadow({ radius: 14, color: '#16000000', offsetY: 6 })
      .onClick(() => {
        this.onNewChat();
      })

      Blank()

      Stack({ alignContent: Alignment.Center }) {
        SidebarGlyph({ kind: 'settings' })
      }
        .width(46)
        .height(46)
        .backgroundColor(CARD)
        .borderRadius(23)
        .shadow({ radius: 14, color: '#16000000', offsetY: 6 })
        .onClick(() => {
          this.onOpenSettings();
        })
    }
    .width('100%')
    .height(56)
    .alignItems(VerticalAlign.Center)
    .zIndex(2)
  }

  @Builder
  private SignedOutFooter() {
    Button(RemoteI18n.t('sidebar.signInBitFunAccount'))
      .width('100%')
      .height(48)
      .fontSize(16)
      .fontWeight(FontWeight.Bold)
      .fontColor(CARD)
      .backgroundColor(INK)
      .borderRadius(16)
      .onClick(() => {
        this.onOpenAccount();
      })
  }

  @Builder
  NavRow(label: string, isActive: boolean, action: () => void) {
    Row({ space: 14 }) {
      SidebarGlyph({ kind: 'remote', connectionState: this.connectionState })
      Text(label)
        .fontSize(18)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
        .layoutWeight(1)
    }
    .width('100%')
    .height(46)
    .padding({ left: 12, right: 8 })
    .backgroundColor(isActive ? SOFT : '#00000000')
    .borderRadius(12)
    .onClick(action)
  }

  @Builder
  ArchivedDisclosureRow() {
    Row({ space: 12 }) {
      SymbolGlyph($r('sys.symbol.archivebox'))
        .fontSize(20)
        .fontColor([MUTED])
        .width(22)
        .height(22)
      Text(RemoteI18n.t('sidebar.archived'))
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Blank()
      Row({ space: 8 }) {
        Text(`${this.archivedSessionCount()}`)
          .fontSize(12)
          .fontWeight(FontWeight.Medium)
          .fontColor(MUTED)
          .width(24)
          .height(22)
          .textAlign(TextAlign.Center)
          .backgroundColor(SOFT)
          .borderRadius(11)
        Stack({ alignContent: Alignment.Center }) {
          if (this.archivedSessionsExpanded) {
            SymbolGlyph($r('sys.symbol.chevron_up'))
              .fontSize(13)
              .fontColor([MUTED])
          } else {
            SymbolGlyph($r('sys.symbol.chevron_down'))
              .fontSize(13)
              .fontColor([MUTED])
          }
        }
        .width(20)
        .height(22)
      }
      .width(52)
      .height(22)
      .alignItems(VerticalAlign.Center)
    }
    .width('100%')
    .height(46)
    .padding({ left: 12, right: 68 })
    .margin({ top: 8 })
    .backgroundColor(this.archivedSessionsExpanded ? SOFT : '#00000000')
    .borderRadius(10)
    .onClick(() => {
      this.archivedSessionsExpanded = !this.archivedSessionsExpanded;
      this.closeSessionActions();
    })
  }

  @Builder
  PinnedRow(session: RemoteSession) {
    Row({ space: 8 }) {
      SymbolGlyph($r('sys.symbol.checkmark_circle'))
        .fontSize(18).fontColor([MUTED]).width(19).height(19)
      Text(session.title || RemoteI18n.t('sidebar.untitled'))
        .fontSize(15).fontColor(INK).layoutWeight(1)
        .maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
      this.SessionMoreButton(session)
    }
    .width('100%').height(44)
    .padding({ left: 12, right: 4 })
    .backgroundColor(this.selectedSessionId === session.id ? SOFT : '#00000000')
    .borderRadius(10)
    .onClick(() => this.openSession(session))
    .gesture(LongPressGesture({ repeat: false }).onAction(() => this.openSessionActions(session)))
    .bindPopup(this.showCollapseButton && this.activeActionSessionId === session.id, {
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
  RecentRow(obj: RepeatItem<RemoteSession>) {
    Row({ space: 8 }) {
      Text(obj.item.title || RemoteI18n.t('sidebar.untitled'))
        .fontSize(15)
        .fontWeight(FontWeight.Regular)
        .fontColor(INK)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
      this.SessionMoreButton(obj.item)
    }
    .width('100%')
    .height(44)
    .padding({ left: 12, right: 4 })
    .backgroundColor(this.selectedSessionId === obj.item.id || this.activeActionSessionId === obj.item.id ?
      SOFT : '#00000000')
    .borderRadius(10)
    .onClick(() => {
      this.openSession(obj.item);
    })
    .gesture(LongPressGesture({ repeat: false }).onAction(() => this.openSessionActions(obj.item)))
    .bindPopup(this.showCollapseButton && this.activeActionSessionId === obj.item.id, {
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
  private SessionMoreButton(session: RemoteSession) {
    Stack({ alignContent: Alignment.Center }) {
      SidebarGlyph({ kind: 'session_more' })
    }
    .width(34)
    .height(40)
    .opacity(0.62)
    .accessibilityText(RemoteI18n.t('session.actions'))
    .onClick(() => this.openSessionActions(session))
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
      archived: this.actionSessionArchived(),
      canViewDetails: this.actionCapabilities().canViewDetails,
      canArchive: this.actionCapabilities().canArchive,
      canExport: this.actionCapabilities().canExport,
      canDelete: this.actionCapabilities().canDelete,
      onViewDetails: () => this.openActionSessionDetails(),
      onArchive: () => this.archiveActionSession(),
      onExport: () => this.exportActionSession(),
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

  private openSession(item: RemoteSession): void {
    if (item.agentType === 'chat' ||
      this.connectionState === 'connected' || this.connectionState === 'reconnecting') {
      this.onOpenSession(item);
      return;
    }
    this.onEnterCode();
  }

  @Builder
  EmptyRecent() {
    Column() {
      Text(this.sessionSearchQuery.trim().length > 0 ?
        RemoteI18n.t('sidebar.noSearchResult') : RemoteI18n.t('sidebar.emptyRecentTitle'))
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
        .width('100%')
    }
    .width('100%')
    .padding({ top: 8, bottom: 8 })
  }

  private visibleRecentSessions(): RemoteSession[] {
    const query = this.sessionSearchQuery.trim().toLowerCase();
    return this.sessions.filter((session: RemoteSession) => {
      if (session.agentType !== 'chat' || session.status === 'archived' || session.id === this.pinnedSessionId) {
        return false;
      }
      return query.length === 0 || session.title.toLowerCase().indexOf(query) >= 0;
    });
  }

  private visibleArchivedSessions(): RemoteSession[] {
    const query = this.sessionSearchQuery.trim().toLowerCase();
    return this.sessions.filter((session: RemoteSession) => {
      return session.agentType === 'chat' && session.status === 'archived' &&
        (query.length === 0 || session.title.toLowerCase().indexOf(query) >= 0);
    });
  }

  private archivedSessionCount(): number {
    return this.sessions.filter((session: RemoteSession) => {
      return session.agentType === 'chat' && session.status === 'archived';
    }).length;
  }

  private visiblePinnedSessions(): RemoteSession[] {
    const query = this.sessionSearchQuery.trim().toLowerCase();
    return this.sessions.filter((session: RemoteSession) => {
      return session.agentType === 'chat' && session.id === this.pinnedSessionId &&
        session.status !== 'archived' &&
        (query.length === 0 || session.title.toLowerCase().indexOf(query) >= 0);
    });
  }

  private openSessionActions(session: RemoteSession): void {
    this.activeActionSessionId = session.id;
    if (!this.showCollapseButton) {
      this.showSessionActionSheet = true;
    }
  }

  private closeSessionActions(): void {
    this.showSessionActionSheet = false;
    this.activeActionSessionId = '';
  }

  private actionSession(): RemoteSession | undefined {
    return this.sessions.find((session: RemoteSession) => session.id === this.activeActionSessionId);
  }

  private actionSessionTitle(): string {
    const session = this.actionSession();
    return session ? session.title : '';
  }

  private actionSessionArchived(): boolean {
    const session = this.actionSession();
    return session ? session.status === 'archived' : false;
  }

  private actionSessionIsGeneralChat(): boolean {
    const session = this.actionSession();
    return session ? session.agentType === 'chat' : false;
  }

  private actionCapabilities(): SessionActionCapabilities {
    const session = this.actionSession();
    return SessionActionPolicy.resolve(
      SessionActionScope.General,
      session ? session.agentType : '',
      session === undefined
    );
  }

  private archiveActionSession(): void {
    const session = this.actionSession();
    if (session) {
      this.onArchiveSession(session, session.status !== 'archived');
    }
  }

  private exportActionSession(): void {
    const session = this.actionSession();
    if (session) {
      this.onExportSession(session);
    }
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
      height: this.actionSessionIsGeneralChat() ? 380 : 300,
      backgroundColor: '#00000000',
      maskColor: '#44000000',
      showClose: false,
      dragBar: false
    };
  }

  private sessionDetailsSheetOptions(): SheetOptions {
    if (!this.showCollapseButton) {
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

  private isAccountAuthenticated(): boolean {
    return this.accountUserId.trim().length > 0;
  }
}
