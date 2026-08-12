import {
  RecentWorkspaceEntry,
  RemoteImageContext,
  RemoteQuestionAnswerPayload,
  RemoteSession,
  SessionSummary,
  SelectedImageAttachment
} from '../../model/RemoteModels';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { ChatTimelineItem } from '../../services/ChatTimelineProjector';
import { ChatTimelineState } from '../../services/ChatTimelineStore';
import { ClipboardService } from '../../services/ClipboardService';
import { ConnectionErrorPolicy } from '../../services/ConnectionErrorPolicy';
import { ImagePickerService } from '../../services/ImagePickerService';
import { GeneralChatCommandController } from '../../services/general-chat/GeneralChatCommandController';
import { GeneralChatConversationViewModel } from './GeneralChatConversationViewModel';
import { GeneralChatDraftLifecycleController } from '../../services/general-chat/GeneralChatDraftLifecycleController';
import {
  GeneralChatServiceState,
  GeneralChatServiceStatus
} from '../../services/general-chat/GeneralChatServiceState';
import { RemoteChatCommandController } from '../../services/RemoteChatCommandController';
import {
  RemoteChatPollingCursor,
  RemoteChatPollingLifecycleController,
  RemoteChatPollingSnapshot
} from '../../services/RemoteChatPollingLifecycleController';
import { RemoteFileDownloadController } from '../../services/RemoteFileDownloadController';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteLogger } from '../../services/RemoteLogger';
import { RemoteModelController } from '../../services/RemoteModelController';
import { RemoteToolActionController } from '../../services/RemoteToolActionController';
import { RemoteUiState } from '../../services/RemoteUiState';
import { RemoteSessionManager } from '../../services/RemoteSessionManager';
import { RemoteWorkspaceCoordinator } from '../../services/RemoteWorkspaceCoordinator';
import { VoiceInputRouteSnapshot } from '../../services/VoiceInputLifecycleController';
import { AppRootRouteState } from '../navigation/AppRootRouteState';
import { AppRoute, AppRouteContract } from '../navigation/AppRouteContract';
import { GeneralChatPageState } from '../state/GeneralChatPageState';
import { RemoteCreateSessionState } from '../state/RemoteCreateSessionState';
import { RemotePageState } from '../state/RemotePageState';
import { ConversationViewModel } from './ConversationViewModel';
import { AppShellViewModel } from './AppShellViewModel';
import { FilePreviewController } from './FilePreviewController';
import { RemoteConnectionController } from './RemoteConnectionController';
import { RemoteSessionViewModel } from './RemoteSessionViewModel';
import { SettingsController } from './SettingsController';
const GENERAL_CHAT_HOME_DRAFT_ID: string = 'new-chat';

export interface ConversationControllerHooks {
  readonly currentRoute: () => AppRoute;
}

export interface RemoteConversationHooks {
  readonly isConversationContext: (sessionId: string) => boolean;
  readonly isFilePreviewVisible: () => boolean;
  readonly stopVoiceInput: () => Promise<void>;
  readonly showToast: (message: string) => boolean;
  readonly selectAssistantWorkspace: (path: string) => Promise<void>;
}

export interface RemoteConversationDependencies {
  readonly timeline: ConversationViewModel;
  readonly chat: RemoteChatCommandController;
  readonly polling: RemoteChatPollingLifecycleController;
  readonly models: RemoteModelController;
  readonly files: RemoteFileDownloadController;
  readonly tools: RemoteToolActionController;
  readonly connection: RemoteConnectionController;
  readonly imagePicker: ImagePickerService;
  readonly clipboard: ClipboardService;
  readonly sessions: RemoteSessionViewModel;
  readonly sessionManager: RemoteSessionManager;
  readonly workspace: RemoteWorkspaceCoordinator;
  readonly settings: SettingsController;
  readonly appShell: AppShellViewModel;
  readonly filePreview: FilePreviewController;
  readonly generalCommands: GeneralChatCommandController;
  readonly generalConversation: GeneralChatConversationViewModel;
  readonly generalDrafts: GeneralChatDraftLifecycleController;
  readonly hooks: RemoteConversationHooks;
}

/** Owns route-dependent composer and voice presentation state. */
export class ConversationController {
  private readonly general: GeneralChatPageState;
  private readonly remote: RemotePageState;
  private readonly remoteCreate: RemoteCreateSessionState;
  private readonly hooks: ConversationControllerHooks;
  private readonly remoteRuntime?: RemoteConversationDependencies;
  private knownPollVersionValue: number = 0;
  private knownModelCatalogVersion: number = 0;
  private knownRemoteMessageCount: number = 0;
  private isSyncingAfterTurn: boolean = false;
  private remoteCreateWorkspaceLoadVersion: number = 0;

  constructor(
    general: GeneralChatPageState,
    remote: RemotePageState,
    remoteCreate: RemoteCreateSessionState,
    hooks: ConversationControllerHooks,
    remoteRuntime?: RemoteConversationDependencies
  ) {
    this.general = general;
    this.remote = remote;
    this.remoteCreate = remoteCreate;
    this.hooks = hooks;
    this.remoteRuntime = remoteRuntime;
  }

  visibleChatInput(): string {
    const route = this.hooks.currentRoute();
    return route === AppRoute.RemoteCreate ? this.remoteCreate.draft :
      AppRootRouteState.chatInput(route, this.general, this.remote);
  }

  visibleSelectedImages(): SelectedImageAttachment[] {
    return AppRootRouteState.selectedImages(this.hooks.currentRoute(), this.general, this.remote);
  }

  visibleVoiceListening(): boolean {
    const route = this.hooks.currentRoute();
    return route === AppRoute.RemoteCreate ? this.remoteCreate.isVoiceListening :
      AppRootRouteState.voiceListening(route, this.general, this.remote);
  }

  setChatInput(route: AppRoute, value: string): void {
    if (route === AppRoute.RemoteCreate) {
      this.remoteCreate.setDraft(value);
      return;
    }
    AppRootRouteState.setChatInput(route, value, this.general, this.remote);
  }

  addSelectedImages(route: AppRoute, images: SelectedImageAttachment[]): void {
    AppRootRouteState.addSelectedImages(route, images, this.general, this.remote);
  }

  removeSelectedImage(route: AppRoute, imageId: string): void {
    AppRootRouteState.removeSelectedImage(route, imageId, this.general, this.remote);
  }

  setVoiceListening(route: AppRoute, isVoiceListening: boolean): void {
    if (route === AppRoute.RemoteCreate) {
      this.remoteCreate.isVoiceListening = isVoiceListening;
      return;
    }
    AppRootRouteState.setVoiceListening(route, isVoiceListening, this.general, this.remote);
  }

  clearAllVoiceListening(): void {
    this.general.setVoiceListening(false);
    this.remote.setVoiceListening(false);
    this.remoteCreate.isVoiceListening = false;
  }

  visibleBusy(): boolean {
    return this.isGeneralComposerRoute(this.hooks.currentRoute()) ?
      this.general.isBusy : this.remote.isBusy;
  }

  visibleStatusText(): string {
    return this.isGeneralComposerRoute(this.hooks.currentRoute()) ?
      this.general.statusText : this.remote.statusText;
  }

  setVisibleStatusText(statusText: string): void {
    if (this.isGeneralComposerRoute(this.hooks.currentRoute())) {
      this.general.setStatus(statusText);
      return;
    }
    this.remote.setStatusText(statusText);
  }

  voiceInputSnapshot(route: AppRoute): VoiceInputRouteSnapshot {
    if (route === AppRoute.RemoteCreate) {
      return {
        routeId: `${route}`,
        isListening: this.remoteCreate.isVoiceListening,
        isBusy: this.remoteCreate.isSubmitting,
        inputText: this.remoteCreate.draft,
        selectedImageCount: 0
      };
    }
    return AppRootRouteState.snapshot(route, this.visibleBusy(), this.general, this.remote);
  }

  isGeneralComposerRoute(route: AppRoute): boolean {
    return AppRouteContract.isGeneralComposerRoute(route);
  }

  knownPollVersion(): number {
    return this.knownPollVersionValue;
  }

  resetKnownRemoteState(): void {
    this.knownPollVersionValue = 0;
    this.knownModelCatalogVersion = 0;
    this.knownRemoteMessageCount = 0;
  }

  updateKnownMessageCount(pollVersion: number, knownMessageCount: number): void {
    this.knownRemoteMessageCount = knownMessageCount;
    this.updateChatPollingCursor(pollVersion, knownMessageCount);
  }

  updateKnownModelCatalogVersion(version: number): void {
    this.knownModelCatalogVersion = version;
    this.requireRemoteRuntime().polling.updateKnownModelCatalogVersion(version);
  }

  async loadRemoteMessages(): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    await runtime.chat.loadMessages(
      this.remote.activeSession.sessionId || '',
      runtime.hooks.isConversationContext
    );
  }

  async loadRemoteModelCatalog(sessionId: string): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    await runtime.models.loadCatalog(
      sessionId,
      runtime.connection.ensureAvailable(),
      runtime.hooks.isConversationContext
    );
  }

  async selectRemoteModel(modelId: string): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    await runtime.models.selectModel(
      modelId,
      this.remote.activeSession.sessionId || '',
      this.remote.isBusy,
      runtime.connection.ensureAvailable()
    );
  }

  async loadOlderRemoteMessages(): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    await runtime.chat.loadOlderMessages(
      this.remote.activeSession.sessionId || '',
      this.knownPollVersionValue,
      this.remote.hasMoreMessages,
      this.remote.isBusy
    );
  }

  async sendRemoteMessage(): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    if (this.remote.isVoiceListening) {
      await runtime.hooks.stopVoiceInput();
    }
    const rawText = this.remote.chatInput.trim();
    const images = this.remote.selectedImages.slice();
    const text = rawText.length > 0 ? rawText :
      (images.length > 0 ? RemoteI18n.t('chat.analyzeImageDefault') : '');
    const sessionId = this.remote.activeSession.sessionId || '';
    if ((!text && images.length === 0) || !sessionId || this.remote.isBusy ||
      !runtime.connection.ensureAvailable()) {
      return;
    }
    this.remote.clearComposer();
    const localMessage = RemoteUiState.localUserMessage(text, images);
    runtime.timeline.appendOptimisticMessage(localMessage);
    const pendingActiveId = runtime.timeline.setPendingActiveTurn(localMessage.id);
    this.syncRemoteTimeline();
    RemoteLogger.info(`chat send queued session=${this.shortSessionId(sessionId)} pending=${pendingActiveId}`);
    this.startRemotePolling();
    runtime.polling.nudge();
    const imageContexts: RemoteImageContext[] = images.length > 0 ?
      runtime.imagePicker.toRemoteContexts(images) : [];
    await runtime.chat.sendPreparedMessage(
      sessionId,
      text,
      this.remote.activeSession.agentType,
      rawText,
      images,
      imageContexts,
      localMessage.id,
      pendingActiveId,
      this.remote.isBusy,
      true
    );
  }

  async stopRemoteTask(): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    const sessionId = this.remote.activeSession.sessionId || '';
    if (!sessionId) {
      return;
    }
    await runtime.chat.stopTask(
      sessionId,
      this.remote.activeTurnMessage.id,
      this.remoteActiveTurnId(),
      runtime.connection.ensureAvailable()
    );
  }

  async renameRemoteSession(title: string): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    const nextTitle = title.trim();
    if (!this.remote.activeSession.sessionId || nextTitle.length === 0 ||
      nextTitle === this.remote.activeSession.title || this.remote.isBusy) {
      return;
    }
    await runtime.chat.renameActiveSession(
      this.remote.activeSession,
      nextTitle,
      this.remote.isBusy,
      runtime.connection.ensureAvailable()
    );
  }

  async copyRemoteMessage(text: string): Promise<void> {
    if (text.trim().length === 0) {
      return;
    }
    try {
      await this.requireRemoteRuntime().clipboard.writeText(text);
      this.remote.setStatusText(RemoteI18n.t('status.messageCopied'));
    } catch (err) {
      this.remote.setStatusText(ConnectionErrorPolicy.errorText(err));
    }
  }

  async downloadRemoteFile(path: string): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    await runtime.files.download(
      path,
      this.remote.activeSession.sessionId || '',
      this.remote.isBusy,
      runtime.connection.ensureAvailable()
    );
  }

  retryRemoteMessage(text: string): void {
    if (this.remote.isBusy || !this.requireRemoteRuntime().connection.ensureAvailable()) {
      return;
    }
    this.remote.setChatInput(text);
    this.sendRemoteMessage();
  }

  async approveRemoteTool(toolId: string, updatedInput?: Object): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    await runtime.tools.approve(
      toolId, this.remote.activeSession.sessionId || '', runtime.connection.ensureAvailable(), updatedInput
    );
  }

  async rejectRemoteTool(toolId: string): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    await runtime.tools.reject(
      toolId, this.remote.activeSession.sessionId || '', runtime.connection.ensureAvailable()
    );
  }

  async cancelRemoteTool(toolId: string): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    await runtime.tools.cancel(
      toolId, this.remote.activeSession.sessionId || '', runtime.connection.ensureAvailable()
    );
  }

  async answerRemoteQuestion(toolId: string, answers: RemoteQuestionAnswerPayload): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    await runtime.tools.answer(
      toolId, this.remote.activeSession.sessionId || '', runtime.connection.ensureAvailable(), answers
    );
  }

  resetRemoteTimeline(sessionId: string): void {
    const runtime = this.requireRemoteRuntime();
    runtime.timeline.reset(sessionId);
    this.knownPollVersionValue = 0;
    this.syncRemoteTimeline();
  }

  syncRemoteTimeline(): void {
    const runtime = this.requireRemoteRuntime();
    const state: ChatTimelineState = runtime.timeline.snapshotState();
    this.remote.setTimelineProjection(
      state.persistedMessages,
      state.optimisticMessages,
      state.activeTurn || RemoteUiState.emptyActiveTurn(),
      this.remote.hasMoreMessages,
      runtime.timeline.viewState(this.remote.hasMoreMessages)
    );
    this.remote.setModelCatalog(state.modelCatalog, state.selectedModelId);
  }

  startRemotePolling(): void {
    this.requireRemoteRuntime().polling.startActiveSession({
      sessionId: this.remote.activeSession.sessionId || '',
      cursor: this.currentChatPollingCursor(),
      activeTurn: this.remote.activeTurnMessage
    });
  }

  applyRemoteSnapshot(snapshot: RemoteChatPollingSnapshot): void {
    const runtime = this.requireRemoteRuntime();
    if (!runtime.hooks.isConversationContext(snapshot.sessionId)) {
      return;
    }
    runtime.timeline.applySnapshot(snapshot);
    this.syncRemoteTimeline();
    this.knownPollVersionValue = snapshot.cursor.pollVersion;
    this.knownModelCatalogVersion = snapshot.cursor.knownModelCatalogVersion;
    this.knownRemoteMessageCount = snapshot.cursor.knownMessageCount;
    if (snapshot.title.length > 0) {
      this.remote.setActiveSession({
        sessionId: this.remote.activeSession.sessionId,
        title: snapshot.title,
        workspacePath: this.remote.activeSession.workspacePath,
        agentType: this.remote.activeSession.agentType
      });
    }
    if (snapshot.modelCatalog) {
      runtime.models.applyCatalog(snapshot.modelCatalog);
    }
    this.remote.setStatusText(this.hasRunningRemoteTurn()
      ? RemoteI18n.t('status.desktopProcessing')
      : RemoteI18n.t('status.messagesSynced'));
    if (snapshot.shouldSyncAfterTurnEnded) {
      this.syncAfterRemoteTurnEnded();
    }
  }

  hasRunningRemoteTurn(): boolean {
    return this.remote.activeTurnMessage.id.length > 0 &&
      (this.remote.activeTurnMessage.status || '').toLowerCase() === 'active';
  }

  remoteActiveTurnId(): string {
    const active = this.remote.activeTurnMessage;
    if (active.turnId && active.turnId.length > 0) {
      return active.turnId;
    }
    return active.id.indexOf('active-') === 0 ? active.id.slice('active-'.length) : '';
  }

  projectedRemoteTimelineItems(): ChatTimelineItem[] {
    return this.requireRemoteRuntime().timeline.viewState(this.remote.hasMoreMessages);
  }

  async createRemoteSession(agentType: string, inPlace: boolean = false): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    runtime.filePreview.close();
    await runtime.sessions.createSession(
      agentType,
      '',
      inPlace ? (sessionId: string): void => this.routeRemoteSessionInPlace(sessionId) :
        (sessionId: string): void => this.routeCreatedRemoteSession(sessionId)
    );
  }

  openRemoteCreateSession(): void {
    const runtime = this.requireRemoteRuntime();
    if (!runtime.connection.ensureAvailable()) {
      return;
    }
    const deviceId = this.remote.controlTargetDeviceId || this.remote.desktopId;
    const deviceName = this.remote.controlTargetDeviceName || this.remote.desktopName;
    this.remoteCreate.prepare(deviceId, deviceName, this.remote.selectedModelId);
    if (deviceId.length > 0) {
      this.remoteCreate.setDevices([{
        deviceId,
        deviceName: deviceName || deviceId,
        online: true
      }]);
    }
    this.remoteCreate.setWorkspaces(this.remote.recentWorkspaces);
    runtime.appShell.pushRoute(AppRoute.RemoteCreate);
    this.loadRemoteCreateChoices();
    this.loadRemoteCreateModelCatalog();
  }

  closeRemoteCreateSession(): void {
    const runtime = this.requireRemoteRuntime();
    this.remoteCreateWorkspaceLoadVersion += 1;
    runtime.hooks.stopVoiceInput();
    this.remoteCreate.closeMenu();
    runtime.appShell.popRoute(AppRoute.RemoteHome);
  }

  async loadRemoteCreateChoices(): Promise<void> {
    await Promise.all([
      this.loadRemoteCreateDevices(),
      this.loadRemoteCreateWorkspaces()
    ]);
  }

  async loadRemoteCreateModelCatalog(): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    if (this.remote.modelCatalog.models.length > 0) {
      return;
    }
    try {
      const catalog = await runtime.sessionManager.getModelCatalog();
      const selectedModelId = RemoteUiState.selectedModelIdForCatalog(catalog, this.remote.selectedModelId);
      this.remote.setModelCatalog(catalog, selectedModelId);
      this.remoteCreate.setSelectedModelId(selectedModelId);
    } catch (_err) {
      // Model selection remains hidden when the remote does not expose a catalog.
    }
  }

  async loadRemoteCreateDevices(): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    this.remoteCreate.isLoadingDevices = this.remoteCreate.devices.length === 0;
    try {
      const phoneDeviceId = runtime.connection.getDeviceId();
      const accountDevices = await runtime.settings.listCloudAccountDevices();
      const devices = accountDevices.filter((device: CloudAccountDevice): boolean =>
        device.online && device.deviceId !== phoneDeviceId
      );
      const currentId = this.remoteCreate.selectedDeviceId;
      if (currentId.length > 0 &&
        !devices.some((device: CloudAccountDevice): boolean => device.deviceId === currentId)) {
        devices.unshift({
          deviceId: currentId,
          deviceName: this.remoteCreate.selectedDeviceName || currentId,
          online: true
        });
      }
      this.remoteCreate.setDevices(devices);
    } catch (_err) {
      const currentId = this.remoteCreate.selectedDeviceId;
      if (currentId.length > 0) {
        this.remoteCreate.setDevices([{
          deviceId: currentId,
          deviceName: this.remoteCreate.selectedDeviceName || currentId,
          online: true
        }]);
      } else {
        this.remoteCreate.setDevices([]);
      }
      this.remoteCreate.errorText = RemoteI18n.t('remote.create.deviceLoadFailed');
    }
  }

  async loadRemoteCreateWorkspaces(): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    const loadVersion = ++this.remoteCreateWorkspaceLoadVersion;
    const deviceId = this.remoteCreate.selectedDeviceId;
    this.remoteCreate.isLoadingWorkspaces = this.remoteCreate.workspaces.length === 0;
    try {
      const workspaces = await runtime.workspace.recentWorkspaces();
      if (loadVersion !== this.remoteCreateWorkspaceLoadVersion ||
        deviceId !== this.remoteCreate.selectedDeviceId) {
        return;
      }
      this.remoteCreate.setWorkspaces(workspaces);
    } catch (_err) {
      if (loadVersion !== this.remoteCreateWorkspaceLoadVersion ||
        deviceId !== this.remoteCreate.selectedDeviceId) {
        return;
      }
      this.remoteCreate.setWorkspaces([]);
      this.remoteCreate.errorText = RemoteI18n.t('remote.create.workspaceLoadFailed');
    }
  }

  toggleRemoteCreateDevices(): void {
    this.remoteCreate.toggleMenu('devices');
    if (this.remoteCreate.openMenu === 'devices' && this.remoteCreate.devices.length === 0) {
      this.loadRemoteCreateDevices();
    }
  }

  toggleRemoteCreateWorkspaces(): void {
    this.remoteCreate.toggleMenu('workspaces');
    if (this.remoteCreate.openMenu === 'workspaces' && this.remoteCreate.workspaces.length === 0) {
      this.loadRemoteCreateWorkspaces();
    }
  }

  async selectRemoteCreateDevice(device: CloudAccountDevice): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    if (device.deviceId === this.remoteCreate.selectedDeviceId) {
      this.remoteCreate.closeMenu();
      return;
    }
    const draft = this.remoteCreate.draft;
    this.remoteCreate.closeMenu();
    this.remoteCreate.isLoadingWorkspaces = true;
    try {
      await runtime.settings.selectCloudAccountDevice(device, false);
      this.remoteCreate.selectDevice(device);
      this.remoteCreate.setDraft(draft);
      await this.loadRemoteCreateWorkspaces();
    } catch (err) {
      this.remoteCreate.isLoadingWorkspaces = false;
      this.remoteCreate.errorText = err instanceof Error ? err.message :
        RemoteI18n.t('remote.settings.deviceSwitchFailed');
    }
  }

  selectRemoteCreateWorkspace(path: string): void {
    const workspace = this.remoteCreate.workspaces
      .find((item: RecentWorkspaceEntry): boolean => item.path === path);
    this.remoteCreate.selectWorkspace(workspace);
  }

  async submitRemoteCreateSession(): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    const instruction = this.remoteCreate.draft.trim();
    if (instruction.length === 0 || this.remoteCreate.isSubmitting || !runtime.connection.ensureAvailable()) {
      return;
    }
    const context = this.remoteCreate.submissionContext();
    const activeDeviceId = this.remote.controlTargetDeviceId || this.remote.desktopId;
    if (context.deviceId.length === 0 || context.deviceId !== activeDeviceId) {
      this.remoteCreate.errorText = RemoteI18n.t('remote.create.deviceMismatch');
      return;
    }
    this.remoteCreate.isSubmitting = true;
    this.remoteCreate.errorText = '';
    this.remoteCreate.closeMenu();
    try {
      if (context.workspacePath.length > 0) {
        await runtime.sessions.createSessionInWorkspace(
          context.workspacePath,
          this.remote.workspacePath,
          instruction,
          context.agentType,
          undefined,
          this.remoteCreate.selectedModelId
        );
      } else {
        await this.bindAssistantWorkspace();
        await runtime.sessions.createSession(
          context.agentType,
          instruction,
          undefined,
          this.remoteCreate.selectedModelId
        );
      }
      if (runtime.appShell.isRoute(AppRoute.RemoteCreate)) {
        this.remoteCreate.errorText = this.remote.statusText || RemoteI18n.t('remote.create.submitFailed');
      }
    } catch (err) {
      this.remoteCreate.errorText = err instanceof Error ? err.message :
        RemoteI18n.t('remote.create.submitFailed');
    } finally {
      this.remoteCreate.isSubmitting = false;
    }
  }

  /**
   * The chat option creates a Claw session, and the desktop always binds those
   * to its assistant workspace. Follow it there first, otherwise the app stays
   * bound to the code workspace it was on and the new chat is listed, titled
   * and file-scoped as if it had been created inside that workspace.
   */
  private async bindAssistantWorkspace(): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    if (this.remote.workspaceKind === 'assistant') {
      return;
    }
    try {
      const assistants = await runtime.workspace.assistants();
      if (assistants.length === 0) {
        return;
      }
      await runtime.hooks.selectAssistantWorkspace(assistants[0].path);
    } catch (err) {
      RemoteLogger.warn(`assistant workspace bind failed: ${String(err)}`);
    }
  }

  async createRemoteSessionInWorkspace(
    path: string,
    agentType: string = 'code',
    inPlace: boolean = false
  ): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    runtime.filePreview.close();
    await runtime.sessions.createSessionInWorkspace(
      path,
      this.remote.workspacePath,
      '',
      agentType,
      inPlace ? (sessionId: string): void => this.routeRemoteSessionInPlace(sessionId) :
        (sessionId: string): void => this.routeCreatedRemoteSession(sessionId)
    );
  }

  async openRemoteSession(item: RemoteSession, inPlace: boolean = false): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    runtime.filePreview.close();
    await runtime.sessions.openSession(
      item,
      this.remote.workspacePath,
      inPlace ? (sessionId: string): void => this.routeRemoteSessionInPlace(sessionId) :
        (sessionId: string): void => this.routeCreatedRemoteSession(sessionId)
    );
  }

  applyRemoteActiveSession(session: SessionSummary): void {
    const runtime = this.requireRemoteRuntime();
    const current = this.remote.activeSession;
    if (runtime.hooks.isFilePreviewVisible() &&
      (current.sessionId !== session.sessionId || current.workspacePath !== session.workspacePath)) {
      runtime.filePreview.close();
    }
    this.remote.setActiveSession(session);
  }

  async deleteRemoteSession(item: RemoteSession): Promise<void> {
    await this.requireRemoteRuntime().sessions.deleteSession(item, this.remote.workspacePath);
  }

  openHomeSession(session: RemoteSession, inPlace: boolean = false): void {
    this.requireRemoteRuntime().filePreview.close();
    if (session.agentType === 'chat') {
      this.openGeneralSession(session);
      return;
    }
    this.openRemoteSession(session, inPlace);
  }

  async deleteHomeSession(session: RemoteSession): Promise<void> {
    if (session.agentType !== 'chat') {
      await this.deleteRemoteSession(session);
      return;
    }
    await this.requireRemoteRuntime().generalCommands.deleteSession(session, this.general.isBusy);
  }

  activeGeneralChatAsRemoteSession(): RemoteSession {
    const active = this.general.activeSession;
    return {
      id: active.sessionId,
      title: active.title,
      agentType: 'chat',
      status: 'ready',
      updatedAt: '',
      createdAt: '',
      messageCount: this.general.timelineItems.length,
      workspacePath: active.workspacePath
    };
  }

  activeGeneralUploadedFileCount(): number {
    let count = 0;
    this.general.timelineItems.forEach((item: ChatTimelineItem) => {
      if (item.message && item.message.images) {
        count += item.message.images.length;
      }
    });
    return count;
  }

  async archiveHomeSession(session: RemoteSession, archived: boolean): Promise<void> {
    await this.requireRemoteRuntime().generalCommands.archiveSession(session, archived, this.general.isBusy);
  }

  async exportHomeSession(session: RemoteSession): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    await runtime.generalCommands.exportSession(
      session,
      this.general.isBusy,
      async (text: string): Promise<void> => runtime.clipboard.writeText(text)
    );
  }

  async openGeneralSession(item: RemoteSession): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    if (this.general.isBusy) {
      return;
    }
    runtime.polling.stop();
    runtime.generalConversation.stop(false);
    await runtime.generalCommands.openSession(
      item,
      this.general.isBusy,
      async (sessionId: string): Promise<string> => runtime.generalDrafts.restore(sessionId),
      (_sessionId: string): void => runtime.appShell.replaceRouteWithoutAnimation(AppRoute.ChatHome)
    );
  }

  async startGeneralChat(text: string): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    const trimmed = text.trim();
    if (trimmed.length === 0 || this.general.isBusy) {
      return;
    }
    runtime.polling.stop();
    runtime.generalConversation.stop(false);
    runtime.generalDrafts.cancel();
    const created = await runtime.generalCommands.createSession(
      trimmed,
      this.general.isBusy,
      async (): Promise<void> => runtime.generalDrafts.clearHomeNow(),
      (_sessionId: string): void => runtime.appShell.replaceRouteWithoutAnimation(AppRoute.ChatHome)
    );
    if (created) {
      await runtime.generalConversation.sendMessage();
    }
  }

  async sendVisibleMessage(): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    if (runtime.appShell.isGeneralChatVisible()) {
      if ((this.general.activeSession.sessionId || '').length === 0) {
        this.startVisibleGeneralChat();
        return;
      }
      await runtime.generalConversation.sendMessage();
      return;
    }
    await this.sendRemoteMessage();
  }

  async stopVisibleTask(): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    if (runtime.appShell.isGeneralChatVisible()) {
      runtime.generalConversation.stop(true);
      return;
    }
    await this.stopRemoteTask();
  }

  closeActiveChat(): void {
    const runtime = this.requireRemoteRuntime();
    runtime.filePreview.close();
    runtime.hooks.stopVoiceInput();
    if (runtime.appShell.isRoute(AppRoute.GeneralChat)) {
      runtime.generalDrafts.persistVisible(this.general.chatInput);
      runtime.generalConversation.stop(true);
      runtime.appShell.popRoute(AppRoute.ChatHome);
      this.restoreGeneralChatDraft(GENERAL_CHAT_HOME_DRAFT_ID);
      return;
    }
    runtime.polling.stop();
    this.remote.setConversationDismissed(true);
    runtime.appShell.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
  }

  async renameVisibleSession(title: string): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    if (runtime.appShell.isGeneralChatVisible()) {
      await runtime.generalCommands.renameActiveSession(this.general.activeSession, title);
      return;
    }
    await this.renameRemoteSession(title);
  }

  async retryVisibleMessage(text: string): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    if (runtime.appShell.isGeneralChatVisible()) {
      const prepared = await runtime.generalCommands.retryMessage(
        this.general.activeSession.sessionId || '', text, this.general.isBusy
      );
      if (prepared) {
        await runtime.generalConversation.sendMessage();
      }
      return;
    }
    this.retryRemoteMessage(text);
  }

  downloadVisibleFile(path: string): void {
    if (this.requireRemoteRuntime().appShell.isGeneralChatVisible()) {
      this.general.setStatus(RemoteI18n.t('generalChat.fileDownloadMock'));
      return;
    }
    this.downloadRemoteFile(path);
  }

  async selectVisibleModel(modelId: string): Promise<void> {
    const runtime = this.requireRemoteRuntime();
    if (runtime.appShell.isGeneralChatVisible()) {
      await runtime.settings.selectModel(modelId);
      return;
    }
    await this.selectRemoteModel(modelId);
  }

  startVisibleGeneralChat(): void {
    const rawText = this.general.chatInput.trim();
    const text = rawText.length > 0 ? rawText :
      (this.general.selectedImages.length > 0 ? RemoteI18n.t('chat.analyzeImageDefault') : '');
    if (text.length === 0 || this.general.isBusy) {
      return;
    }
    if (this.general.serviceState === GeneralChatServiceState.Unconfigured) {
      const statusText = GeneralChatServiceStatus.userMessage(this.general.serviceState);
      this.general.setStatus(statusText);
      this.showHomeToast(statusText);
      return;
    }
    this.startGeneralChat(text);
  }

  generalChatHomeStatusText(): string {
    if (this.general.serviceState === GeneralChatServiceState.Ready ||
      this.general.serviceState === GeneralChatServiceState.Sending ||
      this.general.serviceState === GeneralChatServiceState.Streaming) {
      return '';
    }
    return GeneralChatServiceStatus.userMessage(this.general.serviceState, this.general.statusText);
  }

  prepareNewGeneralChat(): void {
    const runtime = this.requireRemoteRuntime();
    runtime.hooks.stopVoiceInput();
    runtime.generalConversation.stop(true);
    runtime.generalDrafts.clearHome();
    this.general.clearComposer();
    this.general.clearActiveSession();
    this.resetGeneralTimeline('');
    runtime.appShell.replaceRouteWithoutAnimation(AppRoute.ChatHome);
  }

  onVisibleChatInputChange(route: AppRoute, value: string): void {
    this.setChatInput(route, value);
    if (this.isGeneralComposerRoute(route)) {
      this.requireRemoteRuntime().generalDrafts.scheduleVisible(value);
    }
  }

  visibleGeneralChatDraftId(): string {
    return this.requireRemoteRuntime().appShell.isGeneralChatVisible() ?
      this.general.activeSession.sessionId || GENERAL_CHAT_HOME_DRAFT_ID : '';
  }

  async restoreGeneralChatDraft(draftId: string): Promise<void> {
    this.general.setChatInput(await this.requireRemoteRuntime().generalDrafts.restore(draftId));
  }

  latestUserMessageText(): string {
    if (this.requireRemoteRuntime().appShell.isGeneralChatVisible()) {
      return this.general.latestUserMessageText();
    }
    const candidates = this.remote.persistedMessages.concat(this.remote.optimisticMessages);
    for (let index = candidates.length - 1; index >= 0; index--) {
      if (candidates[index].role === 'user' && candidates[index].text.trim().length > 0) {
        return candidates[index].text;
      }
    }
    return '';
  }

  resetGeneralTimeline(sessionId: string): void {
    this.requireRemoteRuntime().timeline.reset(sessionId);
    this.syncGeneralTimeline();
  }

  syncGeneralTimeline(): void {
    const runtime = this.requireRemoteRuntime();
    const state: ChatTimelineState = runtime.timeline.snapshotState();
    const projectedItems = runtime.timeline.viewState(false);
    this.general.setTimelineProjection(
      state.persistedMessages,
      state.optimisticMessages,
      state.activeTurn || RemoteUiState.emptyActiveTurn(),
      false,
      projectedItems
    );
    const itemSummary = projectedItems.map((item: ChatTimelineItem) => {
      const message = item.message;
      return `${item.type}:${item.id}:${message ? message.status : ''}:${message ? message.text.length : 0}`;
    }).join(',');
    RemoteLogger.info(`general chat projection revision=${this.general.timelineRevision} persisted=${state.persistedMessages.length} active=${state.activeTurn ? state.activeTurn.id : 'none'} items=${itemSummary}`);
  }

  showHomeToast(message: string): void {
    const runtime = this.requireRemoteRuntime();
    if (!runtime.hooks.showToast(message)) {
      this.setVisibleStatusText(message);
    }
  }

  private currentChatPollingCursor(): RemoteChatPollingCursor {
    return {
      pollVersion: this.knownPollVersionValue,
      knownMessageCount: this.knownRemoteMessageCount,
      knownModelCatalogVersion: this.knownModelCatalogVersion
    };
  }

  private updateChatPollingCursor(pollVersion: number, knownMessageCount: number): void {
    this.knownPollVersionValue = pollVersion;
    this.knownRemoteMessageCount = knownMessageCount;
    this.requireRemoteRuntime().polling.updateCursor({
      pollVersion,
      knownMessageCount,
      knownModelCatalogVersion: this.knownModelCatalogVersion
    });
  }

  private async syncAfterRemoteTurnEnded(): Promise<void> {
    if (this.isSyncingAfterTurn) {
      return;
    }
    this.isSyncingAfterTurn = true;
    try {
      await this.loadRemoteMessages();
    } finally {
      this.isSyncingAfterTurn = false;
    }
  }

  private shortSessionId(sessionId: string): string {
    return sessionId.length <= 8 ? sessionId :
      sessionId.slice(0, 4) + '...' + sessionId.slice(sessionId.length - 4);
  }

  routeCreatedRemoteSession(sessionId: string): void {
    const runtime = this.requireRemoteRuntime();
    runtime.filePreview.close();
    this.remote.setConversationDismissed(false);
    if (runtime.appShell.isRoute(AppRoute.RemoteCreate)) {
      runtime.appShell.replaceCurrentRoute(AppRoute.RemoteChat, sessionId);
      return;
    }
    runtime.appShell.pushRoute(AppRoute.RemoteChat, sessionId);
  }

  private routeRemoteSessionInPlace(sessionId: string): void {
    const runtime = this.requireRemoteRuntime();
    runtime.filePreview.close();
    this.remote.setConversationDismissed(false);
    const target = AppRouteContract.remoteSessionDestination(sessionId);
    runtime.appShell.replaceRouteWithoutAnimation(target.name, target.routeParam().sessionId);
  }

  private requireRemoteRuntime(): RemoteConversationDependencies {
    if (!this.remoteRuntime) {
      throw new Error('Remote conversation dependencies are not configured.');
    }
    return this.remoteRuntime;
  }
}
