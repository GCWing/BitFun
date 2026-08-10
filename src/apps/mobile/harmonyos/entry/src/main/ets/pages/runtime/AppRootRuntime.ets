import { RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from '../../services/ConnectionErrorPolicy';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { RemoteLogger } from '../../services/RemoteLogger';
import { RemoteUiState } from '../../services/RemoteUiState';
import { AppRootHostPort } from '../host/AppRootHostAdapter';
import {
  AppNavigationBackAction,
  AppRoute,
  AppRouteContract,
  ConversationSource
} from '../navigation/AppRouteContract';
import {
  AppRootRuntimeComposition,
  ConnectionState
} from './AppRootRuntimeComposition';

export class AppRootRuntime extends AppRootRuntimeComposition {
  constructor(host: AppRootHostPort) {
    super(host);
  }

  async aboutToAppear(): Promise<void> {
    this.syncRemotePageSummary();
    await this.generalChatBootstrapController.restore(this.host.context());
    await this.settingsController.initializeCloudAccount(this.host.context());
    await this.settingsController.refreshModelCatalog();
    await this.restoreIdentity();
    await this.startWatchProvisioning();
  }

  /**
   * Only ask for distributed data sync once the phone actually has a desktop
   * to relay to. A fresh install has nothing to provision a watch with, and a
   * permission prompt at first launch would have no explanation behind it.
   */
  private async startWatchProvisioning(): Promise<void> {
    if (!this.hasRemoteBindingForResume()) {
      return;
    }
    await this.watchProvisionController.start(this.host.context());
  }

  onPageShow(): void {
    RemoteLogger.info(`page show state=${(this.remotePageState.connectionState as ConnectionState)} route=${this.appShellViewModel.currentRoute()}`);
    this.remoteActivityViewModel.resume();
    // Idempotent: covers the pairing that happened after the last cold start.
    void this.startWatchProvisioning();
  }

  onPageHide(): void {
    RemoteLogger.info(`page hide state=${(this.remotePageState.connectionState as ConnectionState)} route=${this.appShellViewModel.currentRoute()}`);
    this.remoteActivityViewModel.invalidate();
    this.remoteConnectionCoordinator.invalidate();
    this.remotePageState.setBusy(false);
    this.generalChatDraftLifecycleController.persistVisible(this.generalChatPageState.chatInput);
  }

  aboutToDisappear(): void {
    this.watchProvisionController.stop();
    this.remoteActivityViewModel.invalidate();
    this.remoteConnectionCoordinator.invalidate();
    this.remotePageState.setBusy(false);
    this.generalChatDraftLifecycleController.persistVisible(this.generalChatPageState.chatInput);
    this.generalChatConversationViewModel.stop(true, 'failed');
    this.generalChatDraftLifecycleController.cancel();
    this.remoteFileDownloadController.cancel();
    this.filePreviewController.close();
    this.voiceInputLifecycleController.cancel(`${this.appShellViewModel.currentRoute()}`, () => {
      this.conversationController.clearAllVoiceListening();
    });
  }

  isRemoteConversationContext(sessionId: string): boolean {
    if (sessionId.length === 0 || this.remotePageState.activeSession.sessionId !== sessionId) {
      return false;
    }
    return this.appShellViewModel.isRoute(AppRoute.RemoteChat) || this.appShellViewModel.isRoute(AppRoute.RemoteHome);
  }

  handleNavigationBack(route: AppRoute): boolean {
    if (this.filePreviewState.visible) {
      this.filePreviewController.close();
      return true;
    }
    const action = this.appShellViewModel.backAction(route);
    if (action === AppNavigationBackAction.CloseSidebar) {
      this.closeAppSidebar();
      return true;
    }
    if (action === AppNavigationBackAction.CloseActiveChat) {
      this.exitActiveChat();
      return true;
    }
    if (action === AppNavigationBackAction.PopRemoteHome) {
      this.appShellViewModel.popRoute(AppRoute.ChatHome);
      return true;
    }
    return false;
  }

  handleRootBack(): boolean {
    if (!this.filePreviewState.visible) {
      return false;
    }
    this.filePreviewController.close();
    return true;
  }


  async restoreIdentity(): Promise<void> {
    if (this.remotePageState.controlTargetType === 'account_device') {
      return;
    }
    await this.remoteConnectionController.restore(this.host.context());
  }

  async connect(autoReconnect: boolean = false, accountPassword: string = ''): Promise<void> {
    await this.remoteConnectionController.connect(autoReconnect, accountPassword);
    await this.settingsController.persistDelegatedAccountSession();
  }

  async reconnect(): Promise<void> {
    if (this.remotePageState.controlTargetType === 'account_device') {
      await this.settingsController.restoreCloudTarget(
        this.remotePageState.controlTargetDeviceId,
        this.remotePageState.controlTargetDeviceName
      );
      return;
    }
    await this.remoteConnectionController.reconnect();
  }

  async disconnect(clearPairing: boolean): Promise<void> {
    this.filePreviewController.invalidate();
    await this.remoteConnectionController.disconnect(clearPairing);
  }

  syncRemotePageSummary(): void {
    if (this.remotePageState.statusText.length === 0) {
      this.remotePageState.setStatusText(RemoteI18n.t('status.waitingConnection'));
    }
    if (this.remotePageState.workspaceName.length === 0) {
      this.remotePageState.setWorkspace(RemoteI18n.t('status.notConnected'), '', '', '', 'normal');
    }
  }

  failRemoteConnection(err: Object): void {
    this.remotePageState.setStatusText(ConnectionErrorPolicy.errorText(err));
    this.remotePageState.setConnectionState(ConnectionState.Failed);
    this.remoteActivityViewModel.stopHeartbeat();
  }

  async selectWorkspace(path: string): Promise<void> {
    this.filePreviewController.close();
    await this.remoteWorkspaceViewModel.selectWorkspace(path);
  }

  async selectAssistant(path: string): Promise<void> {
    this.filePreviewController.close();
    await this.remoteWorkspaceViewModel.selectAssistant(path);
  }

  openAppSidebar(): void {
    this.host.animate(230, () => {
      this.appShellState.setSidebarVisible(true);
    });
  }

  closeAppSidebar(): void {
    this.host.animate(210, () => {
      this.appShellState.setSidebarVisible(false);
    });
  }

  enterCodeEntry(): void {
    if (this.settingsController.hasCloudAccountSession() && this.remotePageState.accountUserId.trim().length > 0) {
      this.appShellState.setConnectSheetVisible(true);
      return;
    }
    if (RemoteUiState.canUseRemote((this.remotePageState.connectionState as ConnectionState))) {
      this.appShellState.setConnectSheetVisible(false);
      this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
      return;
    }
    this.appShellState.setConnectSheetVisible(true);
  }

  async switchWideConversationSource(source: ConversationSource): Promise<void> {
    if (AppRouteContract.conversationSource(this.appShellViewModel.currentRoute()) === source) {
      return;
    }
    if (this.conversationController.visibleVoiceListening()) {
      await this.stopVoiceInput(false);
    }
    if (source === ConversationSource.General) {
      this.remoteChatPollingLifecycleController.stop();
      this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.ChatHome);
      return;
    }
    this.generalChatDraftLifecycleController.persistVisible(this.generalChatPageState.chatInput);
    const activeRemoteSessionId = this.remotePageState.isConversationDismissed ? '' :
      (this.remotePageState.activeSession.sessionId || '');
    const target = AppRouteContract.routeForConversationSource(
      source,
      RemoteUiState.canUseRemote((this.remotePageState.connectionState as ConnectionState)),
      activeRemoteSessionId
    );
    this.appShellViewModel.replaceRouteWithoutAnimation(
      target.name,
      target.hasSessionParam() ? target.routeParam().sessionId : ''
    );
    if (target.name === AppRoute.RemoteChat) {
      this.conversationController.startRemotePolling();
      await this.conversationController.loadRemoteMessages();
    }
  }

  /**
   * Compact counterpart of switchWideConversationSource. Switching source is a
   * change of context, not a command to start something: it resumes the session
   * the user was last in, and otherwise rests on the Remote landing surface
   * rather than opening the create composer for them.
   */
  async switchCompactConversationSource(source: ConversationSource): Promise<void> {
    this.closeAppSidebar();
    if (AppRouteContract.conversationSource(this.appShellViewModel.currentRoute()) === source) {
      return;
    }
    if (this.conversationController.visibleVoiceListening()) {
      await this.stopVoiceInput(false);
    }
    if (source === ConversationSource.General) {
      this.remoteChatPollingLifecycleController.stop();
      this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.ChatHome);
      return;
    }
    this.generalChatDraftLifecycleController.persistVisible(this.generalChatPageState.chatInput);
    const activeRemoteSessionId = RemoteUiState.canUseRemote((this.remotePageState.connectionState as ConnectionState)) &&
      !this.remotePageState.isConversationDismissed ?
      (this.remotePageState.activeSession.sessionId || '') : '';
    if (activeRemoteSessionId.length === 0) {
      this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
      return;
    }
    this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteChat, activeRemoteSessionId);
    this.conversationController.startRemotePolling();
    await this.conversationController.loadRemoteMessages();
  }

  enterCompactLayout(): void {
    const sessionId = this.remotePageState.activeSession.sessionId || '';
    if (this.appShellViewModel.isRoute(AppRoute.RemoteHome) &&
      !this.remotePageState.isConversationDismissed && sessionId.length > 0) {
      this.appShellViewModel.pushRoute(AppRoute.RemoteChat, sessionId, false);
    }
  }

  /**
   * Exit control for an open conversation. Leaving a remote conversation on a
   * compact layout lands on Remote Home with no visible session list, so reveal
   * the drawer that owns navigation there. Compact chats have no back button of
   * their own, so this runs for the system back gesture.
   */
  exitActiveChat(): void {
    const revealSidebar = !this.appShellState.wideLayout &&
      this.appShellViewModel.isRoute(AppRoute.RemoteChat);
    this.conversationController.closeActiveChat();
    if (revealSidebar) {
      this.openAppSidebar();
    }
  }

  openRemoteControlSettings(): void {
    setTimeout(() => {
      this.appShellState.openSettings('remote');
    }, 180);
  }

  openAddConnectionFromSettings(): void {
    this.appShellState.setSettingsVisible(false);
    setTimeout(() => {
      this.appShellState.setConnectSheetVisible(true);
    }, 220);
  }

  applyDiscoveredWorkspaceSessions(all: RemoteSession[]): void {
    this.remoteWorkspaceSessions = all;
    const current = this.remotePageState.sessions;
    const extras = all.filter((item: RemoteSession) => item.workspacePath !== this.remotePageState.workspacePath);
    this.remotePageState.setSessions(this.mergeSessions(current, extras), this.remotePageState.hasMoreSessions);
  }

  mergeSessions(primary: RemoteSession[], extras: RemoteSession[]): RemoteSession[] {
    const merged = primary.slice();
    extras.forEach((item: RemoteSession) => {
      if (!merged.some((existing: RemoteSession) => existing.id === item.id)) {
        merged.push(item);
      }
    });
    return merged;
  }

  async toggleVoiceInput(): Promise<void> {
    const route = this.appShellViewModel.currentRoute();
    await this.voiceInputLifecycleController.toggle(
      this.host.context(), this.conversationController.voiceInputSnapshot(route)
    );
  }

  async stopVoiceInput(showStatus: boolean): Promise<void> {
    const route = this.appShellViewModel.currentRoute();
    await this.voiceInputLifecycleController.stop(
      this.conversationController.voiceInputSnapshot(route), showStatus
    );
  }

  showVoiceInputError(message: string): void {
    const text = message.length > 0 ? message : RemoteI18n.t('errors.voiceInputUnavailable');
    this.conversationController.setVisibleStatusText(text);
    this.host.showToast(text, 2600);
  }

  async pickImages(): Promise<void> {
    if (this.conversationController.visibleBusy()) {
      return;
    }
    const route = this.appShellViewModel.currentRoute();
    if (this.conversationController.visibleVoiceListening()) {
      await this.stopVoiceInput(false);
    }
    try {
      this.conversationController.setVisibleStatusText(RemoteI18n.t('status.pickImage'));
      const picked = await this.imagePickerService.pickImages(
        3,
        this.conversationController.visibleSelectedImages().length
      );
      if (picked.length === 0) {
        this.conversationController.setVisibleStatusText(RemoteI18n.t('status.noImageSelected'));
        return;
      }
      this.conversationController.addSelectedImages(route, picked);
      this.conversationController.setVisibleStatusText(RemoteI18n.f(
        'status.imagesSelected',
        `${this.conversationController.visibleSelectedImages().length}`
      ));
    } catch (err) {
      this.conversationController.setVisibleStatusText(ConnectionErrorPolicy.errorText(err));
    }
  }

  currentActiveTurnId(): string {
    if (!this.appShellViewModel.isGeneralChatVisible()) {
      return this.conversationController.remoteActiveTurnId();
    }
    const activeTurnMessage = this.generalChatPageState.activeTurnMessage;
    if (activeTurnMessage.turnId && activeTurnMessage.turnId.length > 0) {
      return activeTurnMessage.turnId;
    }
    const activePrefix = 'active-';
    if (activeTurnMessage.id.indexOf(activePrefix) === 0) {
      return activeTurnMessage.id.slice(activePrefix.length);
    }
    return '';
  }

  hasRemoteBindingForResume(): boolean {
    if (this.remotePageState.controlTargetType === 'account_device') {
      return this.remotePageState.accountUserId.trim().length > 0 &&
        this.remotePageState.controlTargetDeviceId.trim().length > 0 &&
        (this.remotePageState.connectionState as ConnectionState) !== ConnectionState.Idle &&
        (this.remotePageState.connectionState as ConnectionState) !== ConnectionState.Disconnected;
    }
    return this.remotePageState.remoteUrl.trim().length > 0 &&
      this.remotePageState.userId.trim().length > 0 &&
      (this.remotePageState.connectionState as ConnectionState) !== ConnectionState.Idle &&
      (this.remotePageState.connectionState as ConnectionState) !== ConnectionState.Parsing &&
      (this.remotePageState.connectionState as ConnectionState) !== ConnectionState.Pairing &&
      (this.remotePageState.connectionState as ConnectionState) !== ConnectionState.Disconnected;
  }

  async reconnectActiveRemote(): Promise<void> {
    if (this.remotePageState.controlTargetType !== 'account_device') {
      await this.connect(true);
      return;
    }
    const targetId = this.remotePageState.controlTargetDeviceId;
    const device = (await this.settingsController.listCloudAccountDevices())
      .find((item: CloudAccountDevice): boolean => item.deviceId === targetId);
    if (!device) {
      throw new Error(RemoteI18n.t('remote.settings.deviceOffline'));
    }
    await this.settingsController.selectCloudAccountDevice(device);
  }

  hasRemoteBindingForCodeHome(): boolean {
    return this.remotePageState.remoteUrl.trim().length > 0 &&
      this.remotePageState.userId.trim().length > 0 &&
      ((this.remotePageState.connectionState as ConnectionState) === ConnectionState.Connected ||
        (this.remotePageState.connectionState as ConnectionState) === ConnectionState.Reconnecting ||
        (this.remotePageState.connectionState as ConnectionState) === ConnectionState.Pairing ||
        (this.remotePageState.connectionState as ConnectionState) === ConnectionState.Parsing);
  }

}
