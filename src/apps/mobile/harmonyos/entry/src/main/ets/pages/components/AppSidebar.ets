import { MobileDesignGeometry, MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import {
  CARD,
  INK,
  LINE,
  MUTED,
  PAGE_BG,
  PAGE_BG_FADE,
  SCRIM,
  SHADOW_FAINT,
  SHADOW_SUBTLE,
  SOFT,
  SUBTLE,
  TRANSPARENT
} from './Theme';
import { SidebarToggleButton } from './SidebarToggleButton';
import { SessionActionPresentation, SessionActionSurface } from './SessionActionSurface';
import { AdaptiveSheetOptions } from './AdaptiveSheetOptions';
import {
  SettingsPlacement,
  SettingsPlacementPolicy,
  SettingsSheetKind
} from '../policy/SettingsPlacementPolicy';
import { SessionActionCapabilities, SessionActionPolicy, SessionActionScope } from '../policy/SessionActionPolicy';
import { SessionDetailsView } from './SessionDetailsView';
import { SidebarGlyph } from './SidebarGlyphs';
import { SidebarConnectionActionPolicy } from '../policy/SidebarConnectionActionPolicy';

/**
 * How many conversations show before the rest go behind an overflow row.
 *
 * The conversations and the workspaces share one scroll. Recent history comes
 * after the device/workspace navigation, so cap each expansion to keep the
 * bottom section browseable without turning the sidebar into an unbounded list.
 */
const RECENT_BATCH: number = 6;

@ComponentV2
export struct AppSidebar {
  @Param sessions: RemoteSession[] = [];
  @Param sessionDetailsPlacement: SettingsPlacement =
    SettingsPlacementPolicy.compactBottom(SettingsSheetKind.SessionDetails);
  @Param pinnedSessionId: string = '';
  @Param selectedSessionId: string = '';
  @Param connectionState: string = 'idle';
  @Param accountUserId: string = '';
  @Param showCollapseButton: boolean = false;
  @Param showViewSettingsButton: boolean = false;
  @Param showWorkspaceSection: boolean = false;
  @Param showConversationSection: boolean = true;
  @Event onClose: () => void = () => {};
  @Event onNewChat: () => void = () => {};
  @Event onEnterCode: () => void = () => {};
  @Event onScanDesktop: () => void = () => {};
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
  @Local visibleRecentCount: number = RECENT_BATCH;
  /**
   * The device/workspace section, rendered before recent conversations.
   *
   * It is a slot rather than a component this file constructs because the
   * sidebar has no business knowing about remote state; the host wires it.
   */
  @BuilderParam contentSlot: () => void = this.EmptySlot;

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

      Stack({ alignContent: Alignment.Bottom }) {
        // Places are the primary navigation, so devices and workspaces come
        // first. Recent conversations are history and stay at the bottom of
        // the same scroll instead of pushing the current working context down.
        Scroll() {
          Column() {
            if (this.showWorkspaceSection) {
              this.contentSlot()
            }
            if (this.showConversationSection) {
              this.ConversationSection()
            }
          }
          .width('100%')
          .alignItems(HorizontalAlign.Start)
          .padding({ bottom: this.usesCompactFooter() ? 84 : 120 })
        }
        .width('100%')
        .height('100%')
        .align(Alignment.TopStart)
        .scrollable(ScrollDirection.Vertical)
        .scrollBar(BarState.Off)

        // Signed-out actions form a conventional footer and retain a fade.
        // Authenticated navigation uses two independent floating controls, so
        // the scroll remains visible all the way to the bottom behind them.
        if (!this.isAccountAuthenticated()) {
          Column()
            .width('100%')
            .height(this.usesCompactFooter() ? 92 : 120)
            .hitTestBehavior(HitTestMode.None)
            .linearGradient({
              angle: 180,
              colors: [[PAGE_BG_FADE, 0.0], [PAGE_BG, 0.62]]
            })
        }

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
  private EmptySlot() {
  }

  @Builder
  ConversationSection() {
    Column() {
      if (this.visiblePinnedSessions().length > 0) {
        Text(RemoteI18n.t('sidebar.pinned'))
          .fontSize(MobileDesignTypography.bodyMedium.size).fontWeight(FontWeight.Medium).fontColor(MUTED)
          .width('100%').margin({ top: 16, bottom: 6 })
        ForEach(this.visiblePinnedSessions(), (session: RemoteSession) => {
          this.PinnedRow(session)
        }, (session: RemoteSession) => session.id)
      }

      Text(RemoteI18n.t('sidebar.conversations'))
        .fontSize(MobileDesignTypography.bodyMedium.size)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
        .width('100%')
        .margin({ top: 16, bottom: 6 })

      if (this.visibleRecentSessions().length === 0) {
        this.EmptyRecent()
      }
      ForEach(this.cappedRecentSessions(), (session: RemoteSession) => {
        this.RecentRow(session)
      }, (session: RemoteSession) => session.id)
      if (this.visibleRecentCount < this.visibleRecentSessions().length) {
        this.MoreConversationsRow()
      }
      if (this.archivedSessionCount() > 0) {
        this.ArchivedDisclosureRow()
      }
      if (this.archivedSessionsExpanded) {
        ForEach(this.visibleArchivedSessions(), (session: RemoteSession) => {
          this.RecentRow(session)
        }, (session: RemoteSession) => session.id)
      }
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
  }

  @Builder
  private MoreConversationsRow() {
    Row({ space: 8 }) {
      Text('···')
        .fontSize(MobileDesignTypography.bodySmall.size)
        .fontColor(MUTED)
      Text(RemoteI18n.f('sidebar.moreSessions',
        String(this.visibleRecentSessions().length - this.visibleRecentCount)))
        .fontSize(MobileDesignTypography.bodySmall.size)
        .fontColor(MUTED)
    }
    .width('100%')
    .height(40)
    .padding({ left: 12 })
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      this.visibleRecentCount += RECENT_BATCH;
    })
  }

  @Builder
  private AuthenticatedHeader() {
    Row() {
      Text('BitFun')
        .fontSize(MobileDesignTypography.headlineMedium.size)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Blank()
      // Right-aligned so the search and collapse controls keep the same
      // position whether or not the source-specific view-settings entry shows.
      Row({ space: 6 }) {
        if (this.showViewSettingsButton) {
          Button() {
            SidebarGlyph({ kind: 'session_more' })
          }
          .width(MobileDesignGeometry.controlTouchSize)
          .height(MobileDesignGeometry.controlTouchSize)
          .padding(0)
          .backgroundColor(TRANSPARENT)
          .borderRadius(12)
          .stateEffect(true)
          .accessibilityText(RemoteI18n.t('remote.actions'))
          .onClick(() => {
            this.onOpenViewSettings();
          })
        }

        Button() {
          SidebarGlyph({ kind: 'search' })
        }
        .width(44)
        .height(44)
        .padding(0)
        .backgroundColor(CARD)
        .border({ width: 0.5, color: LINE })
        .borderRadius(22)
        .shadow({ radius: 18, color: SHADOW_FAINT, offsetY: 5 })
        .stateEffect(true)
        .accessibilityText(RemoteI18n.t('common.search'))
        .onClick(() => {
          this.showSearch = !this.showSearch;
          if (!this.showSearch) {
            this.updateSearchQuery('');
          }
        })

        if (this.showCollapseButton) {
          SidebarToggleButton({
            controlSize: MobileDesignGeometry.controlTouchSize,
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
        .fontSize(MobileDesignTypography.bodyMedium.size)
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
          .fontSize(MobileDesignTypography.bodyLarge.size)
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
          controlSize: MobileDesignGeometry.controlTouchSize,
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
      Button() {
        Row({ space: 8 }) {
          SidebarGlyph({ kind: 'edit' })
          Text(RemoteI18n.t('sidebar.newChat'))
            .fontSize(MobileDesignTypography.titleMedium.size)
            .fontWeight(FontWeight.Medium)
            .fontColor(INK)
        }
      }
      .width(98)
      .height(44)
      .padding(0)
      .backgroundColor(CARD)
      .border({ width: 0.5, color: LINE })
      .borderRadius(22)
      .shadow({ radius: 24, color: SHADOW_SUBTLE, offsetY: 7 })
      .stateEffect(true)
      .onClick(() => {
        this.onNewChat();
      })

      Blank()

      Button() {
        SidebarGlyph({ kind: 'settings' })
      }
        .width(44)
        .height(44)
        .padding(0)
        .backgroundColor(CARD)
        .borderRadius(22)
        .shadow({ radius: 24, color: SHADOW_SUBTLE, offsetY: 7 })
        .stateEffect(true)
        .onClick(() => {
          this.onOpenSettings();
        })
    }
    .width('100%')
    .height(56)
    .padding({ left: 12 })
    .alignItems(VerticalAlign.Center)
    .zIndex(2)
  }

  @Builder
  private SignedOutFooter() {
    Column({ space: 8 }) {
      if (SidebarConnectionActionPolicy.shouldShowScan(this.connectionState)) {
        Button() {
          Text(RemoteI18n.t('sidebar.scanToConnect'))
            .fontSize(MobileDesignTypography.bodyMedium.size)
            .fontWeight(FontWeight.Medium)
            .fontColor(INK)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Clip })
        }
          .width('100%')
          .height(48)
          .padding(0)
          .backgroundColor(CARD)
          .border({ width: 1, color: LINE })
          .borderRadius(16)
          .onClick(() => {
            this.onScanDesktop();
          })
      }

      Button() {
        Text(RemoteI18n.t('sidebar.signInBitFunAccount'))
          .fontSize(MobileDesignTypography.bodyMedium.size)
          .fontWeight(FontWeight.Bold)
          .fontColor(CARD)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Clip })
      }
        .width('100%')
        .height(48)
        .padding(0)
        .backgroundColor(INK)
        .borderRadius(16)
        .onClick(() => {
          this.onOpenAccount();
        })
    }
      .width('100%')
      .zIndex(2)
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
        .fontSize(MobileDesignTypography.bodyMedium.size)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Blank()
      Row({ space: 8 }) {
        Text(`${this.archivedSessionCount()}`)
          .fontSize(MobileDesignTypography.labelSmall.size)
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
    .backgroundColor(this.archivedSessionsExpanded ? SOFT : TRANSPARENT)
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
        .fontSize(MobileDesignTypography.titleSmall.size).fontColor(INK).layoutWeight(1)
        .maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
      this.SessionMoreButton(session)
    }
    .width('100%').height(44)
    .padding({ left: 12, right: 4 })
    .backgroundColor(this.selectedSessionId === session.id ? SOFT : TRANSPARENT)
    .borderRadius(10)
    .onClick(() => this.openSession(session))
    .gesture(LongPressGesture({ repeat: false }).onAction(() => this.openSessionActions(session)))
    .bindPopup(this.showCollapseButton && this.activeActionSessionId === session.id, {
      builder: () => { this.SessionActionPopover() },
      placement: Placement.Right,
      popupColor: TRANSPARENT,
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
  RecentRow(item: RemoteSession) {
    Row({ space: 8 }) {
      Text(item.title || RemoteI18n.t('sidebar.untitled'))
        .fontSize(MobileDesignTypography.titleSmall.size)
        .fontWeight(FontWeight.Regular)
        .fontColor(INK)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
      this.SessionMoreButton(item)
    }
    .width('100%')
    .height(44)
    .padding({ left: 12, right: 4 })
    .backgroundColor(this.selectedSessionId === item.id || this.activeActionSessionId === item.id ?
      SOFT : TRANSPARENT)
    .borderRadius(10)
    .onClick(() => {
      this.openSession(item);
    })
    .gesture(LongPressGesture({ repeat: false }).onAction(() => this.openSessionActions(item)))
    .bindPopup(this.showCollapseButton && this.activeActionSessionId === item.id, {
      builder: () => { this.SessionActionPopover() },
      placement: Placement.Right,
      popupColor: TRANSPARENT,
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
        .fontSize(MobileDesignTypography.bodyMedium.size)
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

  private cappedRecentSessions(): RemoteSession[] {
    const recent = this.visibleRecentSessions();
    return recent.length <= this.visibleRecentCount ? recent : recent.slice(0, this.visibleRecentCount);
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
      backgroundColor: TRANSPARENT,
      maskColor: SCRIM,
      showClose: false,
      dragBar: false
    };
  }

  private sessionDetailsSheetOptions(): SheetOptions {
    return AdaptiveSheetOptions.fromPlacement(this.sessionDetailsPlacement);
  }

  private isAccountAuthenticated(): boolean {
    return this.accountUserId.trim().length > 0;
  }

  private usesCompactFooter(): boolean {
    return this.isAccountAuthenticated() ||
      !SidebarConnectionActionPolicy.shouldShowScan(this.connectionState);
  }
}
