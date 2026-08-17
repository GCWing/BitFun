import display from '@ohos.display';
import mediaQuery from '@ohos.mediaquery';
import { RemoteUiState } from '../../services/RemoteUiState';
import { AppShell } from './AppShell';
import { FilePreviewSurface } from './FilePreviewSurface';
import { PAGE_BG } from './Theme';
import { AppRoute } from '../navigation/AppRouteContract';
import {
  AppRootPresentationActions,
  emptyAppRootPresentationActions
} from '../actions/AppRootPresentationActions';
import { AppShellState } from '../state/AppShellState';
import {
  ConversationLayoutCrease,
  ConversationLayoutPolicy,
  ConversationLayoutRect
} from '../policy/ConversationLayoutPolicy';
import { GeneralChatPageState } from '../state/GeneralChatPageState';
import { RemotePageState } from '../state/RemotePageState';
import { RemoteCreateSessionState } from '../state/RemoteCreateSessionState';
import { FilePreviewState } from '../state/FilePreviewState';
import {
  FilePreviewLayout,
  FilePreviewPlacement,
  FilePreviewPlacementPolicy
} from '../policy/FilePreviewPlacementPolicy';
import { WideLayoutGeometry } from '../layout/WideLayoutGeometry';
import { ConversationRouteSurface } from './ConversationRouteSurface';
import { WideConversationHost } from './WideConversationHost';
import {
  AppConnectSurface,
  AppSettingsSurface,
  AppSidebarSurface
} from './AppRootOverlaySurfaces';
import {
  RemoteSurfaceHost,
  RemoteSurfaceMode,
  RemoteSurfaceState
} from './remote/RemoteSurfaceHost';

function safeFoldStatus(): display.FoldStatus {
  try {
    return display.getFoldStatus();
  } catch (_err) {
    return display.FoldStatus.FOLD_STATUS_UNKNOWN;
  }
}

@ComponentV2
export struct AppRootPresentation {
  @Param shellState: AppShellState = new AppShellState();
  @Param navigationStack: NavPathStack = new NavPathStack();
  @Param remotePageState: RemotePageState = new RemotePageState();
  @Param remoteCreateState: RemoteCreateSessionState = new RemoteCreateSessionState();
  @Param generalPageState: GeneralChatPageState = new GeneralChatPageState();
  @Param filePreviewState: FilePreviewState = new FilePreviewState();
  @Param deviceId: string = '';
  @Local viewportWidth: number = 0;
  @Local viewportHeight: number = 0;
  @Local wideLayoutMatched: boolean = false;
  @Local foldStatus: display.FoldStatus = safeFoldStatus();
  @Local largeScreenLayout: boolean = false;
  @Local wideMasterPaneWidth: number = ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH;
  @Local wideMasterDetailGap: number = 0;
  @Local wideDetailContentOffset: number = 0;
  @Local wideDetailContentWidth: number = 0;
  @Local wideCollapsedDetailContentOffset: number = 0;
  @Local wideCollapsedDetailContentWidth: number = 0;
  @Local wideMasterPaneCollapsed: boolean = false;
  @Local wideMasterPaneMotionActive: boolean = false;
  @Local restoreCollapsedMasterAfterPreview: boolean = false;
  @Local showRemoteViewSettings: boolean = false;
  @Local remoteSurfaceState: RemoteSurfaceState = new RemoteSurfaceState();
  private verticalCreases: ConversationLayoutCrease[] = [];
  private wideQueryListener?: mediaQuery.MediaQueryListener;
  private foldStatusChanged: (status: display.FoldStatus) => void =
    (status: display.FoldStatus): void => {
      this.foldStatus = status;
      this.refreshWideGeometry();
    };
  private foldDisplayModeChanged: (mode: display.FoldDisplayMode) => void =
    (_mode: display.FoldDisplayMode): void => {
      this.refreshWideGeometry();
    };
  private wideQueryChanged: (result: mediaQuery.MediaQueryResult) => void =
    (result: mediaQuery.MediaQueryResult): void => {
      this.wideLayoutMatched = result.matches;
      this.refreshWideGeometry();
    };
  @Param actions: AppRootPresentationActions = emptyAppRootPresentationActions();

  aboutToAppear(): void {
    this.bindResponsiveQueries();
    this.bindFoldStatus();
  }

  aboutToDisappear(): void {
    this.unbindFoldStatus();
    this.unbindResponsiveQueries();
  }

  @Monitor('filePreviewState.visible')
  onFilePreviewVisibilityChanged(): void {
    if (this.filePreviewState.visible && this.wideMasterPaneCollapsed) {
      this.restoreCollapsedMasterAfterPreview = true;
      this.wideMasterPaneCollapsed = false;
    } else if (!this.filePreviewState.visible && this.restoreCollapsedMasterAfterPreview && this.isWideLayout()) {
      this.wideMasterPaneCollapsed = true;
      this.restoreCollapsedMasterAfterPreview = false;
    }
  }

  build() {
    Stack() {
      AppShell({ shellState: this.shellState, useWideLayout: this.isWideLayout(), content: () => { this.NavigationContent(); },
        sidebar: () => { this.SidebarContent(); }, settings: () => { this.SettingsContent(); },
        connect: () => { this.ConnectContent(); }, onCloseSidebar: this.actions.onCloseSidebar })
      if (this.filePreviewPlacement() === FilePreviewPlacement.CompactFullPage) {
        this.FilePreviewPane()
      }
    }.width('100%').height('100%')
    .bindSheet($$this.showRemoteViewSettings, this.RemoteViewSettingsSheet(), this.remoteViewSettingsSheetOptions())
    .onAreaChange((_oldArea: Area, newArea: Area) => {
      const areaWidth = this.areaWidth(newArea.width);
      const areaHeight = this.areaWidth(newArea.height);
      const displaySize = this.currentDisplaySize();
      this.viewportWidth = ConversationLayoutPolicy.resolveLayoutViewportWidth(
        areaWidth,
        areaHeight,
        displaySize[0],
        displaySize[1],
        displaySize[2]
      );
      this.viewportHeight = areaHeight > 0 ? areaHeight : displaySize[1];
      if (this.viewportWidth === Math.max(displaySize[0], displaySize[1]) &&
        areaWidth !== this.viewportWidth) {
        this.viewportHeight = Math.min(displaySize[0], displaySize[1]);
      }
      this.refreshWideGeometry();
    })
  }

  @Builder
  NavigationContent() {
    Stack() {
      Navigation(this.navigationStack) { this.RouteContent(AppRoute.ChatHome) }
        .id('bitfunHomeNavigation')
        .width('100%').height('100%').hideTitleBar(true)
        .mode(NavigationMode.Stack)
        .navBarWidth('100%')
        .minContentWidth(0)
        .navDestination(this.NavigationDestination)
        .visibility(this.isWideLayout() ? Visibility.Hidden : Visibility.Visible)

      if (this.isWideLayout()) {
        Column() {
          this.RouteContent(this.shellState.activeRoute)
        }
        .width('100%')
        .height('100%')
        .backgroundColor(PAGE_BG)
      }
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  NavigationDestination(name: string) {
    NavDestination() { this.RouteContent(name as AppRoute) }.hideTitleBar(true).backgroundColor(PAGE_BG)
      .onBackPressed(() => this.actions.onNavigationBack(name as AppRoute))
  }

  @Builder
  RouteContent(route: AppRoute) {
    if (this.isWideLayout() && this.isConversationRoute(route)) {
      WideConversationHost({
        route,
        shellState: this.shellState,
        remotePageState: this.remotePageState,
        remoteCreateState: this.remoteCreateState,
        generalPageState: this.generalPageState,
        filePreviewState: this.filePreviewState,
        remoteSurfaceState: this.remoteSurfaceState,
        actions: this.actions,
        filePreviewLayout: this.filePreviewLayout(),
        wideMasterPaneWidth: this.wideMasterPaneWidth,
        wideMasterDetailGap: this.wideMasterDetailGap,
        wideDetailContentOffset: this.wideDetailContentOffset,
        wideDetailContentWidth: this.wideDetailContentWidth,
        wideCollapsedDetailContentOffset: this.wideCollapsedDetailContentOffset,
        wideCollapsedDetailContentWidth: this.wideCollapsedDetailContentWidth,
        wideMasterPaneCollapsed: this.wideMasterPaneCollapsed,
        wideMasterPaneMotionActive: this.wideMasterPaneMotionActive,
        onCollapseMasterPane: () => this.collapseWideMasterPane(),
        onRestoreMasterPane: () => this.restoreWideMasterPane(),
        onOpenRemoteViewSettings: () => { this.showRemoteViewSettings = true; }
      })
    } else {
      ConversationRouteSurface({
        route,
        remotePageState: this.remotePageState,
        remoteCreateState: this.remoteCreateState,
        generalPageState: this.generalPageState,
        filePreviewState: this.filePreviewState,
        remoteSurfaceState: this.remoteSurfaceState,
        actions: this.actions,
        showSidebarButton: true,
        // Compact conversations own the drawer, not a back control: Local and
        // Remote both open the sidebar over the chat instead of leaving it.
        showBackButton: false,
        hoverOperateMode: this.isHoverLayout(),
        onRestoreSidebar: () => this.restoreWideMasterPane()
      })
    }
  }

  @Builder
  RemoteViewSettingsSheet() {
    RemoteSurfaceHost({
      mode: RemoteSurfaceMode.Settings,
      remotePageState: this.remotePageState,
      presentationState: this.remoteSurfaceState,
      actions: this.actions,
      onCloseSettings: () => { this.showRemoteViewSettings = false; }
    })
  }

  @Builder
  FilePreviewPane(paneWidth: number = 0) {
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
    .layoutWeight(paneWidth > 0 ? 0 : 1)
    .width(paneWidth > 0 ? paneWidth : '100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
  }

  private isWideLayout(): boolean {
    return this.largeScreenLayout;
  }

  private collapseWideMasterPane(): void {
    if (!this.isWideLayout() || this.filePreviewState.visible) {
      return;
    }
    this.enableWideMasterPaneMotion();
    this.getUIContext().animateTo({ duration: 220, curve: Curve.EaseInOut }, () => {
      this.wideMasterPaneCollapsed = true;
    });
  }

  private restoreWideMasterPane(): void {
    this.enableWideMasterPaneMotion();
    this.getUIContext().animateTo({ duration: 220, curve: Curve.EaseInOut }, () => {
      this.wideMasterPaneCollapsed = false;
      this.restoreCollapsedMasterAfterPreview = false;
    });
  }

  private enableWideMasterPaneMotion(): void {
    this.wideMasterPaneMotionActive = true;
    setTimeout(() => {
      this.wideMasterPaneMotionActive = false;
    }, 240);
  }

  private filePreviewPlacement(): FilePreviewPlacement {
    return this.filePreviewLayout().placement;
  }

  private filePreviewLayout(): FilePreviewLayout {
    return FilePreviewPlacementPolicy.resolveLayout(
      this.filePreviewState.visible,
      this.isWideLayout(),
      this.viewportWidth,
      this.verticalCreases,
      this.wideMasterPaneWidth
    );
  }

  private isConversationRoute(route: AppRoute): boolean {
    return route === AppRoute.ChatHome || route === AppRoute.GeneralChat ||
      route === AppRoute.RemoteHome || route === AppRoute.RemoteCreate || route === AppRoute.RemoteChat;
  }

  private bindResponsiveQueries(): void {
    this.unbindResponsiveQueries();
    try {
      const appMediaQuery = this.getUIContext().getMediaQuery();
      this.wideQueryListener = appMediaQuery.matchMediaSync(
        `screen and (${ConversationLayoutPolicy.MD_MIN_WIDTH} < width)`
      );
      this.wideLayoutMatched = this.wideQueryListener.matches;
      this.wideQueryListener.on('change', this.wideQueryChanged);
    } catch (_err) {
      this.wideLayoutMatched = this.viewportWidth >= ConversationLayoutPolicy.WIDE_LAYOUT_MIN_WIDTH;
      this.unbindResponsiveQueries();
    }
  }

  private bindFoldStatus(): void {
    this.unbindFoldStatus();
    try {
      this.foldStatus = display.getFoldStatus();
      this.refreshWideGeometry();
      display.on('foldStatusChange', this.foldStatusChanged);
      display.on('foldDisplayModeChange', this.foldDisplayModeChanged);
    } catch (_err) {
      this.foldStatus = display.FoldStatus.FOLD_STATUS_UNKNOWN;
      this.unbindFoldStatus();
    }
  }

  private unbindFoldStatus(): void {
    try {
      display.off('foldStatusChange', this.foldStatusChanged);
      display.off('foldDisplayModeChange', this.foldDisplayModeChanged);
    } catch (_err) {
    }
  }

  private unbindResponsiveQueries(): void {
    if (this.wideQueryListener) {
      this.wideQueryListener.off('change', this.wideQueryChanged);
      this.wideQueryListener = undefined;
    }
  }

  private areaWidth(width: Object): number {
    return WideLayoutGeometry.areaLength(width);
  }

  private refreshWideGeometry(): void {
    const wasWideLayout = this.largeScreenLayout;
    const verticalCreases = this.currentVerticalCreases();
    this.verticalCreases = verticalCreases;
    this.largeScreenLayout = ConversationLayoutPolicy.useMasterDetail(
      this.viewportWidth,
      this.wideLayoutMatched,
      ConversationLayoutPolicy.isFoldedStatus(this.foldStatus),
      verticalCreases,
      this.isExpandedFoldable(),
      this.isHoverLayout()
    );
    const geometry = ConversationLayoutPolicy.resolveWideGeometry(
      this.viewportWidth,
      verticalCreases
    );
    this.wideMasterPaneWidth = geometry.masterPaneWidth;
    this.wideMasterDetailGap = geometry.masterDetailGap;
    this.wideDetailContentOffset = geometry.detailContentOffset;
    this.wideDetailContentWidth = geometry.detailContentWidth;
    this.wideCollapsedDetailContentOffset = geometry.collapsedDetailContentOffset;
    this.wideCollapsedDetailContentWidth = geometry.collapsedDetailContentWidth;
    this.actions.onLayoutModeChanged(this.largeScreenLayout);
    if (wasWideLayout && !this.largeScreenLayout) {
      this.actions.onCompactLayoutEntered();
    }
  }

  private isHoverLayout(): boolean {
    return ConversationLayoutPolicy.useHoverOperate(
      this.foldStatus,
      this.viewportWidth,
      this.viewportHeight
    );
  }

  private isExpandedFoldable(): boolean {
    return ConversationLayoutPolicy.isExpandedFoldableStatus(this.foldStatus) && this.isFoldableDevice();
  }

  private currentDisplaySize(): [number, number, number] {
    try {
      const metrics = display.getDefaultDisplaySync();
      return [
        this.getUIContext().px2vp(metrics.width),
        this.getUIContext().px2vp(metrics.height),
        metrics.rotation
      ];
    } catch (_err) {
      return [0, 0, 0];
    }
  }

  private isFoldableDevice(): boolean {
    try {
      return display.isFoldable();
    } catch (_err) {
      return ConversationLayoutPolicy.isExpandedFoldableStatus(this.foldStatus) ||
        ConversationLayoutPolicy.isFoldedStatus(this.foldStatus);
    }
  }

  private currentVerticalCreases(): ConversationLayoutCrease[] {
    if (ConversationLayoutPolicy.isFoldedStatus(this.foldStatus) || this.isHoverLayout()) {
      return [];
    }
    return ConversationLayoutPolicy.effectiveVerticalCreases(
      this.viewportWidth,
      this.reportedVerticalCreases(),
      this.isExpandedFoldable()
    );
  }

  private reportedVerticalCreases(): ConversationLayoutCrease[] {
    try {
      const region = display.getCurrentFoldCreaseRegion();
      const creaseRects = region.creaseRects;
      if (!creaseRects) {
        return [];
      }
      const rects = creaseRects.map((rect: display.Rect): ConversationLayoutRect => {
        return new ConversationLayoutRect(
          this.getUIContext().px2vp(rect.left),
          this.getUIContext().px2vp(rect.top),
          this.getUIContext().px2vp(rect.width),
          this.getUIContext().px2vp(rect.height)
        );
      });
      return ConversationLayoutPolicy.verticalCreasesFromRects(
        this.viewportWidth,
        this.viewportHeight,
        rects
      );
    } catch (_err) {
      return [];
    }
  }

  private remoteViewSettingsSheetOptions(): SheetOptions {
    if (!this.isWideLayout()) {
      return {
        height: 520,
        backgroundColor: '#00000000',
        maskColor: '#44000000',
        showClose: false,
        dragBar: true
      };
    }
    return {
      height: 520,
      width: 560,
      preferType: SheetType.CENTER,
      backgroundColor: '#00000000',
      maskColor: '#44000000',
      showClose: false,
      dragBar: false
    };
  }

  @Builder SidebarContent() {
    AppSidebarSurface({
      shellState: this.shellState,
      remotePageState: this.remotePageState,
      generalPageState: this.generalPageState,
      remoteSurfaceState: this.remoteSurfaceState,
      actions: this.actions,
      onOpenRemoteViewSettings: () => { this.showRemoteViewSettings = true; }
    })
  }

  @Builder SettingsContent() {
    AppSettingsSurface({
      shellState: this.shellState,
      remotePageState: this.remotePageState,
      generalPageState: this.generalPageState,
      deviceId: this.deviceId,
      actions: this.actions
    })
  }

  @Builder ConnectContent() {
    AppConnectSurface({
      remotePageState: this.remotePageState,
      deviceId: this.deviceId,
      openIntent: this.shellState.connectSheetIntent,
      actions: this.actions
    })
  }
}
