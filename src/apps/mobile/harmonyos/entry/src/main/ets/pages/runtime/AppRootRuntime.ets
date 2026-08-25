import { RemoteSession, SessionSummary } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from '../../services/ConnectionErrorPolicy';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { RemoteLogger } from '../../services/RemoteLogger';
import { stampRemoteSessionsDevice } from '../../services/RemoteSessionIdentity';
import { RemoteUiState } from '../../services/RemoteUiState';
import { AppRootHostPort } from '../host/AppRootHostAdapter';
import { CONNECT_INTENT_SCAN } from '../state/AppShellState';
import {
  AppNavigationBackAction,
  AppRoute,
  AppRouteContract
} from '../navigation/AppRouteContract';
import {
  AppRootRuntimeComposition,
  ConnectionState
} from './AppRootRuntimeComposition';

/**
 * How stale an account device presence answer may be before returning to the
 * foreground, or opening the sidebar, refetches it. Short enough that a
 * desktop coming online is noticed in the same sitting, long enough that the
 * two events firing together on a cold start cost one request, not three.
 */
const DEVICE_PRESENCE_MAX_AGE_MS: number = 15000;

export class AppRootRuntime extends AppRootRuntimeComposition {
  constructor(host: AppRootHostPort) {
    super(host);
  }

  async aboutToAppear(): Promise<void> {
    await this.localeController.initialize(this.host.context());
    this.syncRemotePageSummary();
    await this.remoteChatCache.init(this.host.context());
    await this.remoteSessionListCache.init(this.host.context());
    await this.restoreCachedRemoteSessions();
    await this.generalChatBootstrapController.restore(this.host.context());
    await this.settingsController.initializeCloudAccount(this.host.context());
    if (this.settingsController.hasCloudAccountSession()) {
      try {
        await this.settingsController.listCloudAccountDevices();
        await this.deviceDirectoryViewModel.hydrateKnownDevices();
      } catch (err) {
        RemoteLogger.warn(`device directory restore failed: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
    await this.settingsController.refreshModelCatalog();
    await this.restoreIdentity();
    await this.startWatchProvisioning();
  }

  /**
   * Only ask for distributed data sync once the phone actually has something to
   * provision a watch with. A fresh install has nothing, and a permission
   * prompt at first launch would have no explanation behind it.
   *
   * A signed-in account counts on its own, with no desktop in sight: the phone
   * mints the watch's credential itself in that case, so waiting for a live
   * connection would keep the listener down exactly when it is not needed.
   */
  private async startWatchProvisioning(): Promise<void> {
    if (!this.settingsController.hasCloudAccountSession() && !this.hasRemoteBindingForResume()) {
      return;
    }
    await this.watchProvisionController.start(this.host.context());
  }

  onPageShow(): void {
    RemoteLogger.info(`page show state=${(this.remotePageState.connectionState as ConnectionState)} route=${this.appShellViewModel.currentRoute()}`);
    this.remoteActivityViewModel.resume();
    // Idempotent: covers the pairing that happened after the last cold start.
    void this.startWatchProvisioning();
    void this.settingsController.refreshAccountDevicesIfStale(DEVICE_PRESENCE_MAX_AGE_MS);
    this.settingsController.startPresencePolling(DEVICE_PRESENCE_MAX_AGE_MS);
  }

  onPageHide(): void {
    RemoteLogger.info(`page hide state=${(this.remotePageState.connectionState as ConnectionState)} route=${this.appShellViewModel.currentRoute()}`);
    this.settingsController.stopPresencePolling();
    this.remoteActivityViewModel.invalidate();
    this.remoteConnectionCoordinator.invalidate();
    this.remotePageState.setBusy(false);
    this.generalChatDraftLifecycleController.persistVisible(this.generalChatPageState.chatInput);
  }

  aboutToDisappear(): void {
    this.settingsController.stopPresencePolling();
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
    if (this.appShellState.showConnectSheet) {
      this.appShellState.setConnectSheetVisible(false);
      return true;
    }
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
    if (this.appShellState.showConnectSheet) {
      this.appShellState.setConnectSheetVisible(false);
      return true;
    }
    if (!this.filePreviewState.visible) {
      return false;
    }
    this.filePreviewController.close();
    return true;
  }


  async restoreIdentity(): Promise<void> {
    if (this.remotePageState.controlTargetType === 'account_device') {
      await this.restoreAccountDeviceTarget(false);
      return;
    }
    await this.remoteConnectionController.restore(this.host.context());
  }

  /**
   * Reconnects to the desktop the signed-in account was last driving.
   *
   * The scanned-room path restores itself from its pairing snapshot, but the
   * account path used to do nothing at launch: the phone came up holding a
   * session list and a desktop name with no link behind either, and the only
   * way back was for the user to walk the device picker again. Announcing
   * `reconnecting` up front is what keeps that from reading as connected while
   * the device lookup is still in flight.
   */
  private async restoreAccountDeviceTarget(navigateHome: boolean): Promise<void> {
    const deviceId = this.remotePageState.controlTargetDeviceId.trim();
    if (deviceId.length === 0) {
      return;
    }
    this.remotePageState.setConnectionState(ConnectionState.Reconnecting);
    this.remotePageState.setStatusText(RemoteI18n.t('status.restoringConnection'));
    await this.settingsController.restoreCloudTarget(
      deviceId,
      this.remotePageState.controlTargetDeviceName,
      navigateHome
    );
  }

  async connect(autoReconnect: boolean = false, accountPassword: string = ''): Promise<void> {
    await this.remoteConnectionController.connect(autoReconnect, accountPassword);
    await this.settingsController.persistDelegatedAccountSession();
  }

  async reconnect(): Promise<void> {
    if (this.remotePageState.controlTargetType === 'account_device') {
      await this.restoreAccountDeviceTarget(true);
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

  /**
   * A request that failed against a link the phone still has a binding for is a
   * link that dropped, not one that ended, so the heartbeat stays up and its
   * next ping is what notices the desktop coming back. Killing it here is what
   * used to make a momentary drop permanent: `failed` disables every remote
   * action, and nothing was left running to lift it again.
   */
  failRemoteConnection(err: Object): void {
    this.remotePageState.setStatusText(ConnectionErrorPolicy.errorText(err));
    if (this.hasRemoteBindingForResume()) {
      this.remotePageState.setConnectionState(ConnectionState.Reconnecting);
      this.remoteActivityViewModel.startHeartbeat();
      return;
    }
    this.remotePageState.setConnectionState(ConnectionState.Failed);
    this.remoteActivityViewModel.stopHeartbeat();
  }

  async selectWorkspace(path: string): Promise<void> {
    this.filePreviewController.close();
    await this.remoteWorkspaceViewModel.selectWorkspace(path);
  }

  async selectWorkspaceOnDevice(path: string, deviceId?: string): Promise<void> {
    await this.conversationController.ensureRemoteControlTarget(deviceId);
    this.filePreviewController.close();
    const selected = await this.remoteWorkspaceViewModel.selectWorkspace(path);
    if (!selected) {
      return;
    }
    // A session belongs to the workspace it was created in. Keeping the old
    // active summary after this device-scoped switch can reopen that session
    // under the newly selected workspace and makes the sidebar lie about the
    // Agent's execution context.
    this.remotePageState.clearActiveSession();
    if (this.appShellState.wideLayout) {
      await this.enterRemoteSurfaceWide();
    } else {
      await this.enterRemoteSurfaceCompact();
    }
    // enterRemoteSurface* deliberately no-ops when the current route is
    // already remote. A workspace switch is different: its destination is
    // always the new workspace's landing surface, never the old RemoteChat.
    this.appShellViewModel.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
  }

  async selectAssistant(path: string): Promise<void> {
    this.filePreviewController.close();
    await this.remoteWorkspaceViewModel.selectAssistant(path);
  }

  openAppSidebar(): void {
    this.host.animate(230, () => {
      this.appShellState.setSidebarVisible(true);
    });
    // The device rows are only on screen from here, and a desktop that came
    // online since the last look would otherwise stay grey.
    void this.settingsController.refreshAccountDevicesIfStale(DEVICE_PRESENCE_MAX_AGE_MS);
  }

  closeAppSidebar(): void {
    this.host.animate(210, () => {
      this.appShellState.setSidebarVisible(false);
    });
  }

  async enterRemoteSurfaceWide(): Promise<void> {
    if (AppRouteContract.isRemoteRoute(this.appShellViewModel.currentRoute())) {
      return;
    }
    if (this.conversationController.visibleVoiceListening()) {
      await this.stopVoiceInput(false);
    }
    this.generalChatDraftLifecycleController.persistVisible(this.generalChatPageState.chatInput);
    const activeRemoteSessionId = this.remotePageState.isConversationDismissed ? '' :
      (this.remotePageState.activeSession.sessionId || '');
    const target = AppRouteContract.remoteSurfaceDestination(
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
   * Compact counterpart of enterRemoteSurfaceWide. Reaching the remote surface
   * is a change of context, not a command to start something: it resumes the
   * session the user was last in, and otherwise rests on the Remote landing
   * surface rather than opening the create composer for them.
   */
  async enterRemoteSurfaceCompact(): Promise<void> {
    this.closeAppSidebar();
    if (AppRouteContract.isRemoteRoute(this.appShellViewModel.currentRoute())) {
      return;
    }
    if (this.conversationController.visibleVoiceListening()) {
      await this.stopVoiceInput(false);
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

  /**
   * From the settings row that says 「扫描二维码连接」, and so straight to the
   * camera: this is the one entry that names what it opens, and it is also the
   * only way a signed-in phone can reach the scanner at all — every other path
   * stops at the account's device list.
   */
  openAddConnectionFromSettings(): void {
    this.appShellState.setSettingsVisible(false);
    setTimeout(() => {
      this.openConnectSheet(CONNECT_INTENT_SCAN);
    }, 220);
  }

  applyDiscoveredWorkspaceSessions(all: RemoteSession[]): void {
    this.remoteWorkspaceSessions = all;
    const deviceId = this.remotePageState.controlTargetDeviceId || this.remotePageState.desktopId;
    const current = stampRemoteSessionsDevice(this.remotePageState.sessions, deviceId);
    const extras = stampRemoteSessionsDevice(
      all.filter((item: RemoteSession) => item.workspacePath !== this.remotePageState.workspacePath),
      deviceId
    );
    this.publishRemoteSessions(this.mergeSessions(current, extras), this.remotePageState.hasMoreSessions);
  }

  /**
   * Reconciles the workspace row that owns a newly created remote session.
   *
   * Creating from the sidebar deliberately does not switch the desktop's
   * active workspace. A normal session refresh therefore still lists the old
   * active workspace and cannot discover the new row. Publish the confirmed
   * create immediately, then refresh the workspace named by the response.
   */
  async reconcileCreatedRemoteSession(session: SessionSummary): Promise<void> {
    const workspacePath = session.workspacePath.trim();
    if (workspacePath.length === 0 || workspacePath === this.remotePageState.workspacePath) {
      await this.remoteSessionViewModel.refreshSessions();
      return;
    }

    const now = new Date().toISOString();
    const created: RemoteSession = {
      id: session.sessionId,
      title: session.title,
      agentType: session.agentType,
      status: 'active',
      updatedAt: now,
      createdAt: now,
      messageCount: session.initialTurnId ? 1 : 0,
      workspacePath
    };
    this.remoteWorkspaceSessions = this.replaceWorkspaceSessions(
      workspacePath,
      this.mergeSessions([created], this.sessionsForWorkspace(workspacePath))
    );
    this.publishRemoteWorkspaceSessions();

    try {
      const refreshed = await this.sessionManager.listSessionsForWorkspace(workspacePath, 50);
      const containsCreated = refreshed.some((item: RemoteSession): boolean => item.id === session.sessionId);
      this.remoteWorkspaceSessions = this.replaceWorkspaceSessions(
        workspacePath,
        containsCreated ? refreshed : this.mergeSessions([created], refreshed)
      );
      this.publishRemoteWorkspaceSessions();
    } catch (err) {
      // The create already succeeded and is on screen. Keep that confirmed row
      // instead of turning a follow-up list failure into a false create error.
      RemoteLogger.warn(`created session workspace refresh failed path=${workspacePath}: ${String(err)}`);
    }
  }

  private sessionsForWorkspace(path: string): RemoteSession[] {
    return this.remotePageState.sessions.filter((item: RemoteSession): boolean => item.workspacePath === path);
  }

  private replaceWorkspaceSessions(path: string, replacement: RemoteSession[]): RemoteSession[] {
    const preserved = this.remotePageState.sessions.filter((item: RemoteSession): boolean => {
      return item.workspacePath !== path;
    });
    return replacement.concat(preserved);
  }

  private publishRemoteWorkspaceSessions(): void {
    const currentPath = this.remotePageState.workspacePath;
    const current = this.remotePageState.sessions.filter((item: RemoteSession): boolean => {
      return (item.workspacePath || currentPath) === currentPath;
    });
    const extras = this.remoteWorkspaceSessions.filter((item: RemoteSession): boolean => {
      return item.workspacePath !== currentPath;
    });
    this.publishRemoteSessions(this.mergeSessions(current, extras), this.remotePageState.hasMoreSessions);
  }

  /**
   * The single way a session list reaches the screen, so everything the user
   * sees is also what the next launch starts from.
   *
   * The write is deliberately not awaited: the list is on screen either way,
   * and making a repaint wait on disk is the one way this cache could make the
   * app feel slower than it did without it.
   */
  publishRemoteSessions(sessions: RemoteSession[], hasMore: boolean): void {
    const deviceId = this.remotePageState.controlTargetDeviceId || this.remotePageState.desktopId;
    const stamped = stampRemoteSessionsDevice(sessions, deviceId);
    this.remotePageState.setSessions(stamped, hasMore);
    this.remoteSessionListCache.save(stamped, hasMore);
    this.deviceDirectoryViewModel.captureLive(
      deviceId,
      this.remotePageState.recentWorkspaces,
      stamped
    );
  }

  /**
   * Fills Remote Home from disk before anything is connected.
   *
   * Runs against whichever desktop was used last, because the device to key on
   * is not known this early — identity is still being restored, and the
   * auto-reconnect that follows targets exactly that desktop. Anything the
   * desktop reports replaces this, so a stale entry costs a repaint.
   */
  private async restoreCachedRemoteSessions(): Promise<void> {
    if (this.remotePageState.sessions.length > 0) {
      return;
    }
    const cached = await this.remoteSessionListCache.restoreLast();
    // Checked again: a connection may have landed while the read was in flight,
    // and the desktop's answer outranks the stored one.
    if (cached.sessions.length === 0 || this.remotePageState.sessions.length > 0) {
      return;
    }
    RemoteLogger.info(`session list from cache count=${cached.sessions.length}`);
    this.remotePageState.setSessions(cached.sessions, cached.hasMore);
  }

  mergeSessions(primary: RemoteSession[], extras: RemoteSession[]): RemoteSession[] {
    return RemoteUiState.mergeSessions(primary, extras);
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
