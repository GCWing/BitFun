import { MobileDesignTypography } from '../../../generated/MobileDesignTokens';
import { RemoteI18n } from '../../../i18n/RemoteI18n';
import { RemoteSession } from '../../../model/RemoteModels';
import { RemoteLogger } from '../../../services/RemoteLogger';
import { RemoteUiState } from '../../../services/RemoteUiState';
import { RemotePageState } from '../../state/RemotePageState';
import {
  AppRootPresentationActions,
  emptyAppRootPresentationActions
} from '../../actions/AppRootPresentationActions';
import { ConversationViewSettings } from '../ConversationViewSettings';
import { ConversationHeader } from '../ConversationHeader';
import { RemoteCompactHomePolicy } from '../../policy/RemoteCompactHomePolicy';
import { RemoteSessionList } from '../RemoteSessionList';
import { RemoteSessionLoadingView } from '../RemoteSessionLoadingView';
import { SidebarToggleButton } from '../SidebarToggleButton';
import { SessionActionPresentation } from '../SessionActionSurface';
import {
  SettingsPlacement,
  SettingsPlacementPolicy,
  SettingsSheetKind
} from '../../policy/SettingsPlacementPolicy';
import { CARD, STATUS_SUCCESS, INK, LINE, MUTED, PAGE_BG, PRIMARY_ACTION, CONTENT_ON_ACTION, STATUS_DANGER } from '../Theme';

export enum RemoteSurfaceMode {
  Master = 'master',
  CompactHome = 'compact_home',
  Placeholder = 'placeholder',
  Settings = 'settings'
}

/** Shared presentation state for compact and wide Remote surfaces. */
@ObservedV2
export class RemoteSurfaceState {
  @Trace sortMode: string = 'project';
  @Trace workspaceFilter: string = '';
  @Trace agentFilter: string = '';
  @Trace statusFilter: string = '';
  @Trace showWorkspaceMetadata: boolean = false;
  @Trace showUpdatedMetadata: boolean = false;
  @Trace showStatusMetadata: boolean = false;

  setSortMode(value: string): void { this.sortMode = value; }
  setWorkspaceFilter(value: string): void { this.workspaceFilter = value; }
  setAgentFilter(value: string): void { this.agentFilter = value; }
  setStatusFilter(value: string): void { this.statusFilter = value; }
  setWorkspaceMetadata(value: boolean): void { this.showWorkspaceMetadata = value; }
  setUpdatedMetadata(value: boolean): void { this.showUpdatedMetadata = value; }
  setStatusMetadata(value: boolean): void { this.showStatusMetadata = value; }
}

@ComponentV2
export struct RemoteSurfaceHost {
  @Param mode: RemoteSurfaceMode = RemoteSurfaceMode.Master;
  @Param remotePageState: RemotePageState = new RemotePageState();
  @Param presentationState: RemoteSurfaceState = new RemoteSurfaceState();
  @Param actions: AppRootPresentationActions = emptyAppRootPresentationActions();
  @Param showSelectedSession: boolean = false;
  @Param compact: boolean = false;
  @Param wideMasterPaneCollapsed: boolean = false;
  @Param sessionDetailsPlacement: SettingsPlacement =
    SettingsPlacementPolicy.compactBottom(SettingsSheetKind.SessionDetails);
  @Event onOpenSidebar: () => void = () => {};
  @Event onRestoreSidebar: () => void = () => {};
  @Event onCloseSettings: () => void = () => {};

  // The wide layout keeps the master pane inside a routed destination, so a
  // mount here while switching sessions means the navigation stack was torn
  // down for a change that is only page state — the sidebar rebuild this used
  // to pay for. See AppShellViewModel.replaceRouteWithoutAnimation.
  aboutToAppear(): void {
    RemoteLogger.info(
      `remote surface mounted mode=${this.mode} connection=${this.remotePageState.connectionState} sessions=${this.remotePageState.visibleSessions().length}`
    );
  }

  build() {
    if (this.mode === RemoteSurfaceMode.Master) {
      this.MasterContent();
    } else if (this.mode === RemoteSurfaceMode.CompactHome) {
      this.CompactHomeContent();
    } else if (this.mode === RemoteSurfaceMode.Placeholder) {
      this.FlowPlaceholder();
    } else {
      this.SettingsContent();
    }
  }

  @Builder
  private MasterContent() {
    Column() {
      this.StatusRow()
      if (this.isInitialLoading()) {
        RemoteSessionLoadingView()
      } else if (this.canShowSessionList()) {
        RemoteSessionList({
          sessions: this.remotePageState.visibleSessions(),
          query: this.remotePageState.sessionQuery,
          sortMode: this.presentationState.sortMode,
          workspaceFilter: this.presentationState.workspaceFilter,
          agentFilter: this.presentationState.agentFilter,
          statusFilter: this.presentationState.statusFilter,
          workspaceName: this.remotePageState.workspaceName,
          workspacePath: this.remotePageState.workspacePath,
          workspaceKind: this.remotePageState.workspaceKind,
          recentWorkspaces: this.remotePageState.recentWorkspaces,
          actionPresentation: SessionActionPresentation.Popover,
          sessionDetailsPlacement: this.sessionDetailsPlacement,
          showWorkspaceMetadata: this.presentationState.showWorkspaceMetadata,
          showUpdatedMetadata: this.presentationState.showUpdatedMetadata,
          showStatusMetadata: this.presentationState.showStatusMetadata,
          hasMoreSessions: this.remotePageState.hasMoreSessions,
          isBusy: this.remotePageState.conversation.isBusy || this.remotePageState.isLoadingSessions,
          // A pending id means a row was tapped, whether or not the open is slow
          // enough to have raised a skeleton. Keying the highlight off the
          // loading flag instead used to tie selection to how long the load took.
          selectedSessionId: this.remotePageState.pendingSessionId.length > 0 ?
            this.remotePageState.pendingSessionId :
            (this.showSelectedSession ? this.remotePageState.conversation.activeSession.sessionId : ''),
          onCreate: () => this.createSession('code'),
          onCreateAssistantSession: () => this.createAssistantSession(),
          onCreateInWorkspace: (path: string, agentType: string) => this.createSessionInWorkspace(path, agentType),
          onSelectWorkspace: (path: string) => this.actions.onRemoteHome.selectWorkspace(path),
          onOpenSession: (session: RemoteSession) => this.openSession(session),
          onDeleteSession: (session: RemoteSession) => this.actions.onRemoteHome.deleteSession(session),
          onLoadMore: () => this.actions.onRemoteHome.loadMore()
        })
      } else {
        this.DisconnectedState()
      }
    }
    .width('100%')
    .height('100%')
    .alignItems(HorizontalAlign.Start)
    .padding({ bottom: 84 })
  }

  @Builder
  private StatusRow() {
    Row({ space: 6 }) {
      this.StatusIndicator()
      Text(this.statusText())
        .fontSize(MobileDesignTypography.bodyMedium.size)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .layoutWeight(1)
      if (this.canReconnect()) {
        Text(RemoteI18n.t('remote.settings.reconnect'))
          .fontSize(MobileDesignTypography.bodySmall.size)
          .fontWeight(FontWeight.Medium)
          .fontColor(CONTENT_ON_ACTION)
          .height(28)
          .padding({ left: 12, right: 12 })
          .backgroundColor(PRIMARY_ACTION)
          .borderRadius(14)
          .onClick(() => this.actions.onRemoteHome.reconnect())
      }
    }
    .width('100%')
    .margin({ top: 16, bottom: 6 })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private StatusIndicator() {
    if (this.isSyncing()) {
      LoadingProgress().width(14).height(14).color(MUTED)
    } else {
      Stack() {
        Text('')
      }
      .width(7)
      .height(7)
      .backgroundColor(this.statusColor())
      .borderRadius(4)
    }
  }

  @Builder
  private DisconnectedState() {
    Column({ space: 12 }) {
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.desktop')).fontSize(42).fontColor([INK])
      }
      .width(74)
      .height(74)
      .backgroundColor(CARD)
      .borderRadius(24)
      .border({ width: 1, color: LINE })
      Text(RemoteI18n.t('remote.connectTitle'))
        .fontSize(MobileDesignTypography.headlineSmall.size).fontWeight(FontWeight.Bold).fontColor(INK).textAlign(TextAlign.Center)
      Text(RemoteI18n.t('remote.connectText'))
        .fontSize(MobileDesignTypography.bodySmall.size).lineHeight(MobileDesignTypography.bodySmall.lineHeight).fontColor(MUTED).textAlign(TextAlign.Center)
      Text(RemoteI18n.t('connect.connect'))
        .width(136).height(44).fontSize(MobileDesignTypography.titleSmall.size).fontColor(CONTENT_ON_ACTION)
        .backgroundColor(PRIMARY_ACTION).textAlign(TextAlign.Center).borderRadius(22)
        .onClick(() => this.actions.onRemoteHome.connectWorkspace())
    }
    .layoutWeight(1)
    .width('100%')
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .padding({ left: 20, right: 20, bottom: 48 })
  }

  @Builder
  private CompactHomeContent() {
    Column() {
      ConversationHeader({
        title: RemoteI18n.t('remote.title'),
        subtitle: this.compactHeaderContext(),
        titleFallback: RemoteI18n.t('remote.title'),
        showSidebarButton: true,
        onOpenSidebar: this.onOpenSidebar
      })
      if (RemoteCompactHomePolicy.shouldShowConnectHome(this.remotePageState.connectionState)) {
        this.DisconnectedState()
      } else {
        this.CompactEmptyState()
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
  }

  @Builder
  private CompactEmptyState() {
    Column({ space: 10 }) {
      if (this.isInitialLoading()) {
        LoadingProgress().width(28).height(28).color(MUTED).margin({ bottom: 8 })
      }
      Text(this.compactTitle())
        .fontSize(MobileDesignTypography.headlineMedium.size).fontWeight(FontWeight.Bold).fontColor(INK).textAlign(TextAlign.Center)
      Text(this.compactText())
        .fontSize(MobileDesignTypography.bodyMedium.size).lineHeight(MobileDesignTypography.bodyMedium.lineHeight).fontColor(MUTED).maxLines(2)
        .textOverflow({ overflow: TextOverflow.Ellipsis }).textAlign(TextAlign.Center)
        .constraintSize({ maxWidth: 280 })
      // Starting a session needs a desktop on the other end, so while the link
      // is down the button that is actually useful is the one that brings it
      // back up.
      if (this.canReconnect()) {
        Text(RemoteI18n.t('remote.settings.reconnect'))
          .width(148).height(46).fontSize(MobileDesignTypography.titleSmall.size).fontWeight(FontWeight.Medium)
          .fontColor(CONTENT_ON_ACTION).backgroundColor(PRIMARY_ACTION)
          .textAlign(TextAlign.Center).borderRadius(23).margin({ top: 12 })
          .onClick(() => this.actions.onRemoteHome.reconnect())
      } else {
        Text(RemoteI18n.t('remote.startSession'))
          .width(148).height(46).fontSize(MobileDesignTypography.titleSmall.size).fontWeight(FontWeight.Medium)
          .fontColor(CONTENT_ON_ACTION).backgroundColor(PRIMARY_ACTION)
          .textAlign(TextAlign.Center).borderRadius(23).margin({ top: 12 })
          .onClick(() => this.createSession('code'))
      }
    }
    .width('100%').layoutWeight(1).justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center).padding({ left: 24, right: 24, bottom: 56 })
  }

  @Builder
  private FlowPlaceholder() {
    Column() {
      Row({ space: 8 }) {
        if (this.wideMasterPaneCollapsed) {
          SidebarToggleButton({ restore: true, controlSize: 48, onToggle: this.onRestoreSidebar })
        } else {
          Blank().width(48).height(48)
        }
        Column({ space: 4 }) {
          Text(RemoteI18n.t('remote.chats')).fontSize(MobileDesignTypography.headlineMedium.size).fontWeight(FontWeight.Bold).fontColor(INK)
          Text(this.desktopName()).fontSize(MobileDesignTypography.bodySmall.size).fontColor(MUTED).maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
        .layoutWeight(1).alignItems(HorizontalAlign.Center)
        Blank().width(48).height(48)
      }
      .width('100%').height(76).padding({ left: 16, right: 16, top: 14, bottom: 12 })
      .border({ width: { bottom: 1 }, color: LINE })

      Column({ space: 8 }) {
        if (this.isInitialLoading()) {
          LoadingProgress().width(28).height(28).color(MUTED).margin({ bottom: 8 })
        }
        Text(this.placeholderTitle()).fontSize(MobileDesignTypography.headlineLarge.size).fontWeight(FontWeight.Bold).fontColor(INK)
        Text(this.statusText()).fontSize(MobileDesignTypography.bodyMedium.size).fontColor(MUTED).maxLines(2)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .width('100%').layoutWeight(1).justifyContent(FlexAlign.Center)
      .alignItems(HorizontalAlign.Center).padding({ left: 24, right: 24, bottom: 48 })
    }
    .width('100%').height('100%').backgroundColor(PAGE_BG)
  }

  @Builder
  private SettingsContent() {
    ConversationViewSettings({
      sessions: this.remotePageState.visibleSessions(),
      workspaceName: this.remotePageState.workspaceName,
      workspacePath: this.remotePageState.workspacePath,
      workspaceKind: this.remotePageState.workspaceKind,
      recentWorkspaces: this.remotePageState.recentWorkspaces,
      sortMode: this.presentationState.sortMode,
      workspaceFilter: this.presentationState.workspaceFilter,
      agentFilter: this.presentationState.agentFilter,
      statusFilter: this.presentationState.statusFilter,
      showWorkspaceMetadata: this.presentationState.showWorkspaceMetadata,
      showUpdatedMetadata: this.presentationState.showUpdatedMetadata,
      showStatusMetadata: this.presentationState.showStatusMetadata,
      onSortModeChange: (value: string) => this.presentationState.setSortMode(value),
      onWorkspaceFilterChange: (value: string) => this.presentationState.setWorkspaceFilter(value),
      onAgentFilterChange: (value: string) => this.presentationState.setAgentFilter(value),
      onStatusFilterChange: (value: string) => this.presentationState.setStatusFilter(value),
      onWorkspaceMetadataChange: (value: boolean) => this.presentationState.setWorkspaceMetadata(value),
      onUpdatedMetadataChange: (value: boolean) => this.presentationState.setUpdatedMetadata(value),
      onStatusMetadataChange: (value: boolean) => this.presentationState.setStatusMetadata(value),
      onClose: this.onCloseSettings
    })
  }

  private openSession(session: RemoteSession): void {
    if (this.compact) {
      this.actions.onSidebar.openSession(session);
    } else {
      this.actions.onRemoteHome.openSessionInPlace(session);
    }
  }

  private createSession(agentType: string): void {
    if (this.compact) {
      this.actions.onSidebar.close();
      this.actions.onRemoteHome.create(agentType);
    } else {
      this.actions.onRemoteHome.createInPlace(agentType);
    }
  }

  private createSessionInWorkspace(path: string, agentType: string): void {
    if (this.compact) {
      this.actions.onSidebar.close();
      this.actions.onRemoteHome.createInWorkspace(path, agentType);
    } else {
      this.actions.onRemoteHome.createInWorkspaceInPlace(path, agentType);
    }
  }

  private createAssistantSession(): void {
    this.createSession('code');
  }

  /**
   * Whether the surface still owes the user a way back onto a desktop.
   *
   * The connect button used to live only on the disconnected placeholder, so
   * the moment a restored session list took that placeholder's place there was
   * no entry point left anywhere on the surface — a list from the cache reads
   * as "connected" while nothing is. Offering it from the status row instead
   * keeps the two independent: the list says what the phone knows, this says
   * whether the desktop is answering.
   */
  private canReconnect(): boolean {
    return !RemoteUiState.canUseRemote(this.remotePageState.connectionState) && !this.isSyncing();
  }

  private canShowSessionList(): boolean {
    return this.remotePageState.connectionState === 'connected' ||
      this.remotePageState.visibleSessions().length > 0 ||
      this.remotePageState.isLoadingHome || this.remotePageState.isLoadingSessions;
  }

  /**
   * Only the first load gets a skeleton. Once there are sessions to show — from
   * this connection or from the cache a launch restored — connecting is a
   * refresh happening behind a usable list, and replacing that list with
   * placeholder rows would be a step backwards.
   */
  private isInitialLoading(): boolean {
    if (this.remotePageState.sessions.length > 0) {
      return false;
    }
    return this.isSyncing();
  }

  /** Work in progress, whether or not there is already a list behind it. */
  private isSyncing(): boolean {
    return this.remotePageState.isLoadingHome || this.isConnecting();
  }

  private isConnecting(): boolean {
    return this.remotePageState.connectionState === 'parsing' ||
      this.remotePageState.connectionState === 'pairing' ||
      this.remotePageState.connectionState === 'reconnecting';
  }

  private statusText(): string {
    if (this.remotePageState.conversation.statusText.length > 0) {
      return this.remotePageState.conversation.statusText;
    }
    return this.desktopName();
  }

  private statusColor(): ResourceColor {
    if (this.remotePageState.connectionState === 'connected') return STATUS_SUCCESS;
    if (this.remotePageState.connectionState === 'failed' || this.remotePageState.connectionState === 'disconnected') {
      return STATUS_DANGER;
    }
    return MUTED;
  }

  /**
   * Compact Remote Home names the bound desktop under the title, the same
   * context the conversation header carries. Stays empty while disconnected so
   * the connect state does not advertise a stale desktop.
   */
  private compactHeaderContext(): string {
    return RemoteCompactHomePolicy.headerSubtitle(
      this.remotePageState.connectionState,
      this.remotePageState.desktopName
    );
  }

  private desktopName(): string {
    return this.remotePageState.desktopName.length > 0 ? this.remotePageState.desktopName :
      RemoteI18n.t('remote.settings.noDesktop');
  }

  private compactTitle(): string {
    if (this.isInitialLoading()) return RemoteI18n.t('common.loading');
    return this.remotePageState.visibleSessions().length > 0 ?
      RemoteI18n.t('remote.pickSession') : RemoteI18n.t('remote.emptyTitle');
  }

  private compactText(): string {
    if (this.isInitialLoading()) return this.statusText();
    return this.remotePageState.visibleSessions().length > 0 ?
      RemoteI18n.t('remote.pickSessionText') : RemoteI18n.t('remote.emptyText');
  }

  private placeholderTitle(): string {
    if (this.isInitialLoading()) return RemoteI18n.t('common.loading');
    return this.remotePageState.visibleSessions().length > 0 ? '选择会话' : RemoteI18n.t('remote.emptyTitle');
  }
}
