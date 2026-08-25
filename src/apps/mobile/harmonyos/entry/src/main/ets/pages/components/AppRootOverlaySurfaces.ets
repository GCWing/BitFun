import {
  AppRootPresentationActions,
  emptyAppRootPresentationActions
} from '../actions/AppRootPresentationActions';
import { AppRouteContract } from '../navigation/AppRouteContract';
import { AppShellState, CONNECT_INTENT_AUTO } from '../state/AppShellState';
import {
  SettingsPlacement,
  SettingsPlacementPolicy,
  SettingsSheetKind
} from '../policy/SettingsPlacementPolicy';
import { GeneralChatPageState } from '../state/GeneralChatPageState';
import { DeviceDirectoryState } from '../state/DeviceDirectoryState';
import { RemotePageState } from '../state/RemotePageState';
import { AppSidebar } from './AppSidebar';
import { ConnectView } from './ConnectView';
import { RemoteControlSettingsSheet } from './RemoteControlSettingsSheet';
import { RemoteSurfaceState } from './remote/RemoteSurfaceHost';
import { SettingsSheet } from './SettingsSheet';
import { SidebarWorkspaceSection } from './SidebarWorkspaceSection';

@ComponentV2
export struct AppSidebarSurface {
  @Param shellState: AppShellState = new AppShellState();
  @Param sessionDetailsPlacement: SettingsPlacement =
    SettingsPlacementPolicy.compactBottom(SettingsSheetKind.SessionDetails);
  @Param remotePageState: RemotePageState = new RemotePageState();
  @Param deviceDirectoryState: DeviceDirectoryState = new DeviceDirectoryState();
  @Param generalPageState: GeneralChatPageState = new GeneralChatPageState();
  @Param remoteSurfaceState: RemoteSurfaceState = new RemoteSurfaceState();
  @Param actions: AppRootPresentationActions = emptyAppRootPresentationActions();
  @Event onOpenRemoteViewSettings: () => void = () => {};

  build() {
    AppSidebar({
      sessions: this.generalPageState.recentSessions(),
      sessionDetailsPlacement: this.sessionDetailsPlacement,
      pinnedSessionId: this.generalPageState.pinnedSessionId(),
      selectedSessionId: AppRouteContract.isGeneralComposerRoute(this.shellState.activeRoute) ?
        this.generalPageState.conversation.activeSession.sessionId : '',
      connectionState: this.remotePageState.connectionState,
      accountUserId: this.remotePageState.accountUserId,
      showViewSettingsButton: false,
      showWorkspaceSection: true,
      contentSlot: () => {
        this.WorkspaceContent()
      },
      onClose: this.actions.onSidebar.close,
      onNewChat: this.actions.onSidebar.newChat,
      onEnterCode: this.actions.onSidebar.enterCode,
      onScanDesktop: this.actions.onSidebar.scanDesktop,
      onOpenViewSettings: this.onOpenRemoteViewSettings,
      onSearchQueryChange: (query: string) => {
        this.actions.onRemoteHome.queryChanged(query);
      },
      onOpenSettings: this.actions.onSidebar.settings,
      onOpenAccount: this.actions.onSidebar.openAccount,
      onOpenSession: this.actions.onSidebar.openSession,
      onArchiveSession: this.actions.onSidebar.archive,
      onExportSession: this.actions.onSidebar.exportSession,
      onDeleteSession: this.actions.onSidebar.deleteSession
    })
  }

  @Builder
  private WorkspaceContent() {
    SidebarWorkspaceSection({
      directoryState: this.deviceDirectoryState,
      sessions: this.remotePageState.visibleSessions(),
      recentWorkspaces: this.remotePageState.recentWorkspaces,
      workspaceName: this.remotePageState.workspaceName,
      workspacePath: this.remotePageState.workspacePath,
      workspaceKind: this.remotePageState.workspaceKind,
      connectionState: this.remotePageState.connectionState,
      controlTargetType: this.remotePageState.controlTargetType,
      controlTargetDeviceId: this.remotePageState.controlTargetDeviceId,
      controlTargetDeviceName: this.remotePageState.controlTargetDeviceName ||
        this.remotePageState.desktopName,
      selectedSessionId: this.remoteSelectedSessionId(),
      runningSessionId: this.remoteRunningSessionId(),
      workspacePickerPlacement: this.sessionDetailsPlacement,
      onOpenSession: this.actions.onSidebar.openSession,
      onCreateInWorkspace: (deviceId: string, path: string) => {
        this.actions.onSidebar.close();
        this.actions.onRemoteHome.createInWorkspace(path, 'code', deviceId);
      },
      onOpenWorkspace: (deviceId: string, path: string) => {
        this.actions.onRemoteHome.selectWorkspace(path, deviceId);
      },
      onToggleDevice: this.actions.onRemoteHome.toggleDirectoryDevice,
      onRetryDevice: this.actions.onRemoteHome.retryDirectoryDevice,
      onAdd: this.actions.onSidebar.addDesktop
    })
  }

  private remoteSelectedSessionId(): string {
    if (this.remotePageState.pendingSessionId.length > 0) {
      return this.remotePageState.pendingSessionId;
    }
    return this.remotePageState.activeSession.sessionId;
  }

  private remoteRunningSessionId(): string {
    return this.remotePageState.hasRunningActiveTurn() ? this.remotePageState.activeSession.sessionId : '';
  }
}

@ComponentV2
export struct AppSettingsSurface {
  @Param shellState: AppShellState = new AppShellState();
  @Param remotePageState: RemotePageState = new RemotePageState();
  @Param generalPageState: GeneralChatPageState = new GeneralChatPageState();
  @Param deviceId: string = '';
  @Param actions: AppRootPresentationActions = emptyAppRootPresentationActions();

  build() {
    if (this.shellState.settingsMode === 'remote' || this.shellState.settingsMode === 'account') {
      RemoteControlSettingsSheet({
        desktopName: this.remotePageState.desktopName,
        desktopId: this.remotePageState.desktopId,
        userId: this.remotePageState.userId,
        accountUsername: this.remotePageState.accountUsername,
        accountUserId: this.remotePageState.accountUserId,
        deviceId: this.deviceId,
        controlTargetType: this.remotePageState.controlTargetType,
        controlTargetDeviceId: this.remotePageState.controlTargetDeviceId,
        connectionState: this.remotePageState.connectionState,
        statusText: this.remotePageState.conversation.statusText,
        isBusy: this.remotePageState.conversation.isBusy,
        onClose: this.actions.onSettings.close,
        onOpenAccount: this.actions.onSettings.openAccount,
        onAddConnection: this.actions.onSettings.addConnection,
        cloudLogin: this.actions.onSettings.cloudLogin,
        cloudLogout: this.actions.onSettings.cloudLogout,
        cloudListDevices: this.actions.onSettings.cloudListDevices,
        cloudSelectDevice: this.actions.onSettings.cloudSelectDevice,
        getPermissionMode: this.actions.onSettings.getPermissionMode,
        setPermissionMode: this.actions.onSettings.setPermissionMode,
        openAccountOnAppear: this.shellState.settingsMode === 'account',
        onDisconnect: this.actions.onSettings.disconnect,
        onReconnect: this.actions.onSettings.reconnect
      })
    } else {
      SettingsSheet({
        sheetState: this.shellState.settingsSheet,
        generalChatApiUrl: this.generalPageState.apiUrl,
        generalChatModelName: this.generalPageState.modelName,
        hasGeneralChatApiKey: this.generalPageState.hasApiKey,
        generalChatModelCatalog: this.generalPageState.conversation.modelCatalog,
        selectedGeneralChatModelId: this.generalPageState.conversation.selectedModelId,
        accountUsername: this.remotePageState.accountUsername,
        authenticatedUserId: this.remotePageState.accountUserId,
        deviceId: this.deviceId,
        onOpenAccount: this.actions.onSettings.openAccount,
        onTestGeneralChatConfig: this.actions.onSettings.testGeneral,
        onSaveGeneralChatConfig: this.actions.onSettings.saveGeneral,
        onSelectGeneralChatModel: this.actions.onSettings.selectGeneralModel,
        onSetLanguage: this.actions.onSettings.setLanguage,
        onClose: this.actions.onSettings.close
      })
    }
  }
}

@ComponentV2
export struct AppConnectSurface {
  @Param remotePageState: RemotePageState = new RemotePageState();
  @Param deviceId: string = '';
  @Param openIntent: string = CONNECT_INTENT_AUTO;
  @Param actions: AppRootPresentationActions = emptyAppRootPresentationActions();

  build() {
    ConnectView({
      remoteUrl: this.remotePageState.remoteUrl,
      userId: this.remotePageState.userId,
      statusText: this.remotePageState.conversation.statusText,
      connectionState: this.remotePageState.connectionState,
      connectionFailureKind: this.remotePageState.connectionFailureKind,
      isBusy: this.remotePageState.conversation.isBusy,
      isConnected: this.remotePageState.connectionState === 'connected',
      desktopName: this.remotePageState.desktopName,
      deviceId: this.deviceId,
      accountUserId: this.remotePageState.accountUserId,
      controlTargetDeviceId: this.remotePageState.controlTargetDeviceId,
      requiresAccountAuth: this.remotePageState.requiresAccountAuth,
      accountUsername: this.remotePageState.accountUsername,
      openIntent: this.openIntent,
      onBack: this.actions.onConnect.back,
      onOpenAccount: this.actions.onConnect.openAccount,
      onConnect: this.actions.onConnect.connect,
      onRemoteUrlChange: this.actions.onConnect.urlChanged,
      onUserIdChange: this.actions.onConnect.userChanged,
      onRemoteUrlDetected: this.actions.onConnect.detected,
      onRemoteUrlInputVisibleChange: this.actions.onConnect.inputVisible,
      cloudListDevices: this.actions.onConnect.cloudListDevices,
      cloudSelectDevice: this.actions.onConnect.cloudSelectDevice
    })
    .width('100%')
    .height('100%')
  }
}
