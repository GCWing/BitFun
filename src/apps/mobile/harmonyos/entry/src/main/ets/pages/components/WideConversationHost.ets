import { RemoteUiState } from '../../services/RemoteUiState';
import {
  AppRootPresentationActions,
  emptyAppRootPresentationActions
} from '../actions/AppRootPresentationActions';
import { WideLayoutGeometry } from '../layout/WideLayoutGeometry';
import { AppRoute, ConversationSource } from '../navigation/AppRouteContract';
import { FilePreviewLayout, FilePreviewPlacement } from '../policy/FilePreviewPlacementPolicy';
import { AppShellState } from '../state/AppShellState';
import { FilePreviewState } from '../state/FilePreviewState';
import { GeneralChatPageState } from '../state/GeneralChatPageState';
import { RemoteCreateSessionState } from '../state/RemoteCreateSessionState';
import { RemotePageState } from '../state/RemotePageState';
import { AppSidebar } from './AppSidebar';
import { ConversationRouteSurface } from './ConversationRouteSurface';
import { FilePreviewSurface } from './FilePreviewSurface';
import {
  RemoteSurfaceHost,
  RemoteSurfaceMode,
  RemoteSurfaceState
} from './remote/RemoteSurfaceHost';
import { SidebarToggleButton } from './SidebarToggleButton';
import { FLOATING_PANEL_BG, LINE, PAGE_BG } from './Theme';

const WIDE_DETAIL_CONTENT_MAX_WIDTH: number = 920;

@ComponentV2
export struct WideConversationHost {
  @Param route: AppRoute = AppRoute.ChatHome;
  @Param shellState: AppShellState = new AppShellState();
  @Param remotePageState: RemotePageState = new RemotePageState();
  @Param remoteCreateState: RemoteCreateSessionState = new RemoteCreateSessionState();
  @Param generalPageState: GeneralChatPageState = new GeneralChatPageState();
  @Param filePreviewState: FilePreviewState = new FilePreviewState();
  @Param remoteSurfaceState: RemoteSurfaceState = new RemoteSurfaceState();
  @Param actions: AppRootPresentationActions = emptyAppRootPresentationActions();
  @Param filePreviewLayout: FilePreviewLayout = new FilePreviewLayout(FilePreviewPlacement.Hidden);
  @Param wideMasterPaneWidth: number = 0;
  @Param wideMasterDetailGap: number = 0;
  @Param wideDetailContentOffset: number = 0;
  @Param wideDetailContentWidth: number = 0;
  @Param wideCollapsedDetailContentOffset: number = 0;
  @Param wideCollapsedDetailContentWidth: number = 0;
  @Param wideMasterPaneCollapsed: boolean = false;
  @Param wideMasterPaneMotionActive: boolean = false;
  @Event onCollapseMasterPane: () => void = () => {};
  @Event onRestoreMasterPane: () => void = () => {};
  @Event onOpenRemoteViewSettings: () => void = () => {};

  build() {
    if (this.route === AppRoute.ChatHome || this.route === AppRoute.GeneralChat) {
      this.GeneralChatContent();
    } else if (this.showsRemoteConversation() &&
      this.filePreviewLayout.placement === FilePreviewPlacement.WideFocusSplit) {
      this.RemotePreviewFocusContent();
    } else if (this.showsRemoteConversation()) {
      this.RemoteChatContent();
    } else if (this.route === AppRoute.RemoteHome) {
      this.RemoteHomeContent();
    } else {
      this.RemoteCreateContent();
    }
  }

  @Builder
  private GeneralChatContent() {
    Row() {
      if (!this.wideMasterPaneCollapsed) {
        this.MasterPane(ConversationSource.General, false)
        this.MasterDetailGap()
      }
      this.ConversationDetail(false)
    }
    .width('100%').height('100%').backgroundColor(PAGE_BG)
  }

  @Builder
  private RemoteHomeContent() {
    Row() {
      if (!this.wideMasterPaneCollapsed) {
        this.MasterPane(ConversationSource.Remote, false)
        this.MasterDetailGap()
      }
      Column() {
        RemoteSurfaceHost({
          mode: RemoteSurfaceMode.Placeholder,
          remotePageState: this.remotePageState,
          presentationState: this.remoteSurfaceState,
          actions: this.actions,
          wideMasterPaneCollapsed: this.wideMasterPaneCollapsed,
          onRestoreSidebar: this.onRestoreMasterPane
        })
      }
      .layoutWeight(1).height('100%').backgroundColor(PAGE_BG)
    }
    .width('100%').height('100%').backgroundColor(PAGE_BG)
  }

  @Builder
  private RemoteCreateContent() {
    Row() {
      if (!this.wideMasterPaneCollapsed) {
        this.MasterPane(ConversationSource.Remote, false)
        this.MasterDetailGap()
      }
      this.ConversationDetail(false)
    }
    .width('100%').height('100%').backgroundColor(PAGE_BG)
  }

  @Builder
  private MasterPane(source: ConversationSource, showSelectedSession: boolean) {
    Column() {
      Column() {
        AppSidebar({
          sessions: source === ConversationSource.Remote ? [] : this.generalPageState.recentSessions(),
          pinnedSessionId: this.generalPageState.pinnedSessionId(),
          selectedSessionId: source === ConversationSource.Remote ? '' :
            this.generalPageState.conversation.activeSession.sessionId,
          connectionState: this.remotePageState.connectionState,
          accountUserId: this.remotePageState.accountUserId,
          activeSection: source === ConversationSource.Remote ? 'remote' : 'chat',
          showConversationSourceSwitcher: true,
          showCollapseButton: true,
          showViewSettingsButton: source === ConversationSource.Remote,
          showCustomContent: source === ConversationSource.Remote,
          conversationSource: source,
          contentSlot: () => {
            this.RemoteMasterContent(showSelectedSession)
          },
          onClose: this.actions.onSidebar.close,
          onNewChat: source === ConversationSource.Remote ?
            this.actions.onRemoteHome.createAssistant : this.actions.onSidebar.newChat,
          onEnterCode: () => this.actions.onWideConversationSource(ConversationSource.Remote),
          onConversationSource: this.actions.onWideConversationSource,
          onCollapse: this.onCollapseMasterPane,
          onOpenViewSettings: this.onOpenRemoteViewSettings,
          onSearchQueryChange: (query: string) => {
            if (source === ConversationSource.Remote) this.actions.onRemoteHome.queryChanged(query);
          },
          onOpenSettings: source === ConversationSource.Remote ?
            this.actions.onRemoteHome.openSettings : this.actions.onSidebar.settings,
          onOpenAccount: this.actions.onSidebar.openAccount,
          onOpenSession: this.actions.onSidebar.openSession,
          onArchiveSession: this.actions.onSidebar.archive,
          onExportSession: this.actions.onSidebar.exportSession,
          onDeleteSession: this.actions.onSidebar.deleteSession
        })
      }
      .width('100%').height('100%').backgroundColor(FLOATING_PANEL_BG)
      .borderRadius(18).clip(true)
      .shadow({ radius: 24, color: '#14000000', offsetX: 4, offsetY: 8 })
    }
    .width(WideLayoutGeometry.masterPaneWidth(this.filePreviewLayout, this.wideMasterPaneWidth))
    .height('100%').padding({ left: 10, right: 6, top: 10, bottom: 10 }).backgroundColor(PAGE_BG)
    .transition(this.wideMasterPaneMotionActive ?
      TransitionEffect.translate({ x: -28, y: 0 }).combine(TransitionEffect.opacity(0))
        .animation({ duration: 220, curve: Curve.EaseInOut }) : TransitionEffect.opacity(1))
  }

  @Builder
  private RemoteMasterContent(showSelectedSession: boolean) {
    RemoteSurfaceHost({
      mode: RemoteSurfaceMode.Master,
      remotePageState: this.remotePageState,
      presentationState: this.remoteSurfaceState,
      actions: this.actions,
      showSelectedSession,
      compact: false
    })
  }

  @Builder
  private RemoteChatContent() {
    if (this.filePreviewLayout.placement === FilePreviewPlacement.WideTriplePane) {
      Row() {
        this.MasterPane(ConversationSource.Remote, true)
        this.PaneGap(this.filePreviewLayout.masterConversationGap)
        this.ConversationDetail(false, this.filePreviewLayout.conversationPaneWidth)
        this.PaneGap(this.filePreviewLayout.conversationPreviewGap)
        this.FilePreviewPane(this.filePreviewLayout.previewPaneWidth)
      }
      .width('100%').height('100%').backgroundColor(PAGE_BG)
    } else {
      Row() {
        if (!this.wideMasterPaneCollapsed) {
          this.MasterPane(ConversationSource.Remote, true)
          this.MasterDetailGap()
        }
        this.ConversationDetail(false)
      }
      .width('100%').height('100%').backgroundColor(PAGE_BG)
    }
  }

  @Builder
  private RemotePreviewFocusContent() {
    Row() {
      this.ConversationDetail(false, this.filePreviewLayout.conversationPaneWidth)
      this.PaneGap(this.filePreviewLayout.conversationPreviewGap)
      this.FilePreviewPane(this.filePreviewLayout.previewPaneWidth)
    }
    .width('100%').height('100%').backgroundColor(PAGE_BG)
  }

  @Builder
  private FilePreviewPane(paneWidth: number) {
    Column() {
      FilePreviewSurface({
        state: this.filePreviewState,
        remoteAvailable: RemoteUiState.canUseRemote(this.remotePageState.connectionState),
        downloadPath: this.remotePageState.downloadingFilePath,
        downloadedPath: this.remotePageState.downloadedFilePath,
        downloadStatus: this.remotePageState.fileDownloadStatus,
        onClose: this.actions.onFilePreview.close,
        onRefresh: this.actions.onFilePreview.refresh,
        onDownload: this.actions.onFilePreview.download,
        onOpenLink: this.actions.onFilePreview.openLink
      })
    }
    .width(paneWidth).height('100%').backgroundColor(PAGE_BG)
  }

  @Builder
  private ConversationDetail(showBackButton: boolean, paneWidth: number = 0) {
    if (paneWidth > 0) {
      Column() {
        this.RouteSurface(showBackButton)
      }
      .width(paneWidth).height('100%').constraintSize({ maxWidth: WIDE_DETAIL_CONTENT_MAX_WIDTH })
      .backgroundColor(PAGE_BG)
    } else {
      Stack({ alignContent: Alignment.TopStart }) {
        Row() {
          if (this.currentDetailOffset() > 0) Blank().width(this.currentDetailOffset())
          Row() {
            Column() { this.RouteSurface(showBackButton) }
              .width('100%').height('100%').constraintSize({ maxWidth: WIDE_DETAIL_CONTENT_MAX_WIDTH })
              .backgroundColor(PAGE_BG)
          }
          .width(this.currentDetailWidth() > 0 ? this.currentDetailWidth() : '100%')
          .height('100%').justifyContent(FlexAlign.Center)
          if (this.currentDetailOffset() > 0) Blank().layoutWeight(1)
        }
        .width('100%').height('100%').justifyContent(FlexAlign.Center).backgroundColor(PAGE_BG)

        if (this.wideMasterPaneCollapsed) {
          SidebarToggleButton({ restore: true, controlSize: 44, onToggle: this.onRestoreMasterPane })
            .position({ x: 12, y: 12 }).zIndex(2)
            .hitTestBehavior(HitTestMode.Default)
            .transition(TransitionEffect.scale({ x: 0.9, y: 0.9 }).combine(TransitionEffect.opacity(0))
              .animation({ duration: 180, curve: Curve.EaseOut }))
        }
      }
      .layoutWeight(1).height('100%').backgroundColor(PAGE_BG)
    }
  }

  @Builder
  private RouteSurface(showBackButton: boolean) {
    ConversationRouteSurface({
      route: this.route,
      remotePageState: this.remotePageState,
      remoteCreateState: this.remoteCreateState,
      generalPageState: this.generalPageState,
      filePreviewState: this.filePreviewState,
      remoteSurfaceState: this.remoteSurfaceState,
      actions: this.actions,
      showSidebarButton: false,
      showBackButton,
      useWidePresentation: true,
      contentHorizontalOffset: this.collapsedDetailVisualBias(),
      onRestoreSidebar: this.onRestoreMasterPane
    })
  }

  @Builder
  private MasterDetailGap() {
    if (this.wideMasterDetailGap > 0) {
      Row() {}.width(this.wideMasterDetailGap).height('100%').backgroundColor(LINE)
    }
  }

  @Builder
  private PaneGap(width: number) {
    if (width > 0) {
      Row() {}.width(width).height('100%').backgroundColor(LINE)
    }
  }

  private showsRemoteConversation(): boolean {
    return this.route === AppRoute.RemoteChat;
  }

  private currentDetailOffset(): number {
    return WideLayoutGeometry.detailOffset(
      this.wideMasterPaneCollapsed,
      this.wideDetailContentOffset,
      this.wideCollapsedDetailContentOffset
    );
  }

  private currentDetailWidth(): number {
    return WideLayoutGeometry.detailWidth(
      this.wideMasterPaneCollapsed,
      this.wideDetailContentWidth,
      this.wideCollapsedDetailContentWidth
    );
  }

  private collapsedDetailVisualBias(): number {
    return WideLayoutGeometry.collapsedVisualBias(
      this.wideMasterPaneCollapsed,
      this.wideCollapsedDetailContentOffset,
      this.wideCollapsedDetailContentWidth,
      WIDE_DETAIL_CONTENT_MAX_WIDTH,
      72
    );
  }
}
