import {
  AssistantEntry,
  ChatMessage,
  RecentWorkspaceEntry,
  RemoteModelCatalog,
  RemoteModelConfig,
  RemoteSession,
  SelectedImageAttachment,
  SessionSummary
} from '../../model/RemoteModels';
import { ChatTimelineItem, ChatTimelineRevisionTracker } from '../../services/ChatTimelineProjector';
import { RemoteUiState } from '../../services/RemoteUiState';

/**
 * Observable projection for the Remote home page.
 * Connection, persistence and transport ownership stay outside this object.
 */
@ObservedV2
export class RemotePageState {
  @Trace desktopName: string = 'MacBook Pro';
  @Trace desktopId: string = '';
  @Trace remoteUrl: string = '';
  @Trace userId: string = '';
  @Trace requiresAccountAuth: boolean = false;
  @Trace accountUsername: string = '';
  @Trace authenticatedUserId: string = '';
  @Trace statusText: string = '';
  @Trace connectionState: string = 'idle';
  @Trace connectionFailureKind: string = '';
  @Trace isBusy: boolean = false;
  @Trace showRemoteUrlInput: boolean = false;
  @Trace workspaceName: string = '';
  @Trace workspacePath: string = '';
  @Trace workspaceBranch: string = '';
  @Trace workspaceKind: string = 'normal';
  @Trace assistantId: string = '';
  @Trace recentWorkspaces: RecentWorkspaceEntry[] = [];
  @Trace assistants: AssistantEntry[] = [];
  @Trace showWorkspacePicker: boolean = false;
  @Trace showAssistantPicker: boolean = false;
  @Trace sessions: RemoteSession[] = [];
  @Trace activeSession: SessionSummary = RemotePageState.emptySession();
  @Trace persistedMessages: ChatMessage[] = [];
  @Trace optimisticMessages: ChatMessage[] = [];
  @Trace activeTurnMessage: ChatMessage = RemoteUiState.emptyActiveTurn();
  @Trace hasMoreMessages: boolean = false;
  @Trace timelineItems: ChatTimelineItem[] = [];
  @Trace timelineRevision: number = 0;
  @Trace modelCatalog: RemoteModelCatalog = RemoteUiState.emptyModelCatalog();
  @Trace selectedModelId: string = '';
  @Trace downloadingFilePath: string = '';
  @Trace downloadedFilePath: string = '';
  @Trace fileDownloadStatus: string = '';
  @Trace chatInput: string = '';
  @Trace selectedImages: SelectedImageAttachment[] = [];
  @Trace isVoiceListening: boolean = false;
  @Trace sessionQuery: string = '';
  @Trace hasMoreSessions: boolean = false;
  @Trace isLoadingSessions: boolean = false;
  @Trace sessionErrorText: string = '';
  private readonly timelineRevisionTracker: ChatTimelineRevisionTracker = new ChatTimelineRevisionTracker();

  setQuery(query: string): void {
    this.sessionQuery = query;
  }

  setDesktopName(desktopName: string): void {
    this.desktopName = desktopName;
  }

  setDesktopIdentity(desktopName: string, desktopId: string): void {
    this.desktopName = desktopName;
    this.desktopId = desktopId;
  }

  setPairingInput(remoteUrl: string, userId: string, showRemoteUrlInput: boolean): void {
    this.remoteUrl = remoteUrl;
    this.userId = userId;
    this.showRemoteUrlInput = showRemoteUrlInput;
  }

  setAccountPairing(requiresAccountAuth: boolean, accountUsername: string): void {
    this.requiresAccountAuth = requiresAccountAuth;
    this.accountUsername = accountUsername;
  }

  setRemoteUrl(remoteUrl: string): void {
    this.remoteUrl = remoteUrl;
  }

  setUserId(userId: string): void {
    this.userId = userId;
  }

  setAuthenticatedUserId(authenticatedUserId: string): void {
    this.authenticatedUserId = authenticatedUserId;
  }

  setStatusText(statusText: string): void {
    this.statusText = statusText;
  }

  setConnectionState(connectionState: string): void {
    this.connectionState = connectionState;
  }

  setConnectionFailureKind(connectionFailureKind: string): void {
    this.connectionFailureKind = connectionFailureKind;
  }

  setBusy(isBusy: boolean): void {
    this.isBusy = isBusy;
  }

  setRemoteUrlInputVisible(visible: boolean): void {
    this.showRemoteUrlInput = visible;
  }

  setWorkspace(
    workspaceName: string,
    workspacePath: string,
    assistantId: string,
    workspaceBranch: string = '',
    workspaceKind: string = 'normal'
  ): void {
    this.workspaceName = workspaceName;
    this.workspacePath = workspacePath;
    this.assistantId = assistantId;
    this.workspaceBranch = workspaceBranch;
    this.workspaceKind = workspaceKind;
  }

  setRecentWorkspaces(recentWorkspaces: RecentWorkspaceEntry[]): void {
    this.recentWorkspaces = recentWorkspaces.slice();
  }

  setAssistants(assistants: AssistantEntry[]): void {
    this.assistants = assistants.slice();
  }

  setWorkspacePickerVisible(visible: boolean): void {
    this.showWorkspacePicker = visible;
  }

  setAssistantPickerVisible(visible: boolean): void {
    this.showAssistantPicker = visible;
  }

  closePickers(): void {
    this.showWorkspacePicker = false;
    this.showAssistantPicker = false;
  }

  setSessions(sessions: RemoteSession[], hasMore: boolean): void {
    this.sessions = sessions.slice();
    this.hasMoreSessions = hasMore;
    this.sessionErrorText = '';
  }

  setActiveSession(session: SessionSummary): void {
    this.activeSession = {
      sessionId: session.sessionId,
      title: session.title,
      workspacePath: session.workspacePath,
      agentType: session.agentType,
      initialTurnId: session.initialTurnId
    };
  }

  clearActiveSession(): void {
    this.activeSession = RemotePageState.emptySession();
    this.clearTimeline();
    this.setModelCatalog(RemoteUiState.emptyModelCatalog(), '');
  }

  setTimelineProjection(
    persistedMessages: ChatMessage[],
    optimisticMessages: ChatMessage[],
    activeTurnMessage: ChatMessage,
    hasMoreMessages: boolean,
    timelineItems: ChatTimelineItem[]
  ): void {
    this.timelineRevision = this.timelineRevisionTracker.update(timelineItems);
    this.persistedMessages = persistedMessages.slice();
    this.optimisticMessages = optimisticMessages.slice();
    this.activeTurnMessage = activeTurnMessage.id.length > 0 ?
      RemotePageState.copyMessage(activeTurnMessage) :
      RemoteUiState.emptyActiveTurn();
    this.hasMoreMessages = hasMoreMessages;
    this.timelineItems = timelineItems.slice();
  }

  clearTimeline(): void {
    this.persistedMessages = [];
    this.optimisticMessages = [];
    this.activeTurnMessage = RemoteUiState.emptyActiveTurn();
    this.hasMoreMessages = false;
    this.timelineItems = [];
    this.timelineRevision = this.timelineRevisionTracker.reset();
  }

  setModelCatalog(modelCatalog: RemoteModelCatalog, selectedModelId: string): void {
    this.modelCatalog = RemotePageState.copyModelCatalog(modelCatalog);
    this.selectedModelId = selectedModelId;
  }

  setDownloadStatus(downloadingFilePath: string, downloadedFilePath: string, fileDownloadStatus: string): void {
    this.downloadingFilePath = downloadingFilePath;
    this.downloadedFilePath = downloadedFilePath;
    this.fileDownloadStatus = fileDownloadStatus;
  }

  clearDownloadingFilePath(): void {
    this.downloadingFilePath = '';
  }

  clearDownloadStatus(): void {
    this.downloadingFilePath = '';
    this.downloadedFilePath = '';
    this.fileDownloadStatus = '';
  }

  setChatInput(chatInput: string): void {
    this.chatInput = chatInput;
  }

  setSelectedImages(selectedImages: SelectedImageAttachment[]): void {
    this.selectedImages = selectedImages.slice();
  }

  addSelectedImages(selectedImages: SelectedImageAttachment[]): void {
    this.selectedImages = this.selectedImages.concat(selectedImages);
  }

  removeSelectedImage(imageId: string): void {
    this.selectedImages = this.selectedImages.filter((image: SelectedImageAttachment) => image.id !== imageId);
  }

  clearComposer(): void {
    this.chatInput = '';
    this.selectedImages = [];
  }

  setVoiceListening(isVoiceListening: boolean): void {
    this.isVoiceListening = isVoiceListening;
  }

  setLoading(loading: boolean): void {
    this.isLoadingSessions = loading;
  }

  setError(errorText: string): void {
    this.sessionErrorText = errorText;
    this.isLoadingSessions = false;
  }

  clear(): void {
    this.sessions = [];
    this.hasMoreSessions = false;
    this.isLoadingSessions = false;
    this.sessionErrorText = '';
  }

  clearWorkspaceActions(): void {
    this.recentWorkspaces = [];
    this.assistants = [];
    this.closePickers();
  }

  visibleSessions(): RemoteSession[] {
    const query = this.sessionQuery.trim().toLowerCase();
    return this.sessions.filter((item: RemoteSession) => {
      if (item.id.length === 0 || item.status === 'archived') {
        return false;
      }
      return query.length === 0 || item.title.toLowerCase().indexOf(query) >= 0;
    });
  }

  hasRunningActiveTurn(): boolean {
    return this.activeTurnMessage.id.length > 0 &&
      (this.activeTurnMessage.status || '').toLowerCase() === 'active';
  }

  private static emptySession(): SessionSummary {
    return {
      sessionId: '',
      title: '',
      workspacePath: '',
      agentType: 'code'
    };
  }

  private static copyMessage(message: ChatMessage): ChatMessage {
    return {
      id: message.id,
      role: message.role,
      text: message.text,
      status: message.status,
      renderVersion: message.renderVersion,
      turnId: message.turnId,
      detail: message.detail,
      timestamp: message.timestamp,
      thinking: message.thinking,
      tools: message.tools ? message.tools.slice() : undefined,
      items: message.items ? message.items.slice() : undefined,
      images: message.images ? message.images.slice() : undefined
    };
  }

  private static copyModelCatalog(modelCatalog: RemoteModelCatalog): RemoteModelCatalog {
    return {
      version: modelCatalog.version,
      models: modelCatalog.models.map((model): RemoteModelConfig => {
        return {
          id: model.id,
          name: model.name,
          provider: model.provider,
          base_url: model.base_url,
          model_name: model.model_name,
          context_window: model.context_window,
          enabled: model.enabled,
          capabilities: model.capabilities.slice(),
          enable_thinking_process: model.enable_thinking_process,
          reasoning_mode: model.reasoning_mode,
          reasoning_effort: model.reasoning_effort
        };
      }),
      default_models: {
        primary: modelCatalog.default_models.primary,
        fast: modelCatalog.default_models.fast,
        search: modelCatalog.default_models.search,
        image_understanding: modelCatalog.default_models.image_understanding
      },
      session_model_id: modelCatalog.session_model_id
    };
  }
}
