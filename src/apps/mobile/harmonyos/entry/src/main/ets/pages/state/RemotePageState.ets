import {
  AssistantEntry,
  ChatMessage,
  RecentWorkspaceEntry,
  RemoteModelCatalog,
  RemoteSession,
  SelectedImageAttachment,
  SessionSummary
} from '../../model/RemoteModels';
import { ChatTimelineItem, ObservableChatTimelineItem } from '../../model/ChatTimelineModels';
import { RemoteUiState } from '../../services/RemoteUiState';
import { ConversationCoreState } from './ConversationCoreState';

/**
 * Observable projection for the Remote home page.
 * Connection, persistence and transport ownership stay outside this object.
 */
@ObservedV2
export class RemotePageState {
  @Trace conversation: ConversationCoreState = new ConversationCoreState('code');
  @Trace desktopName: string = '';
  @Trace desktopId: string = '';
  @Trace remoteUrl: string = '';
  @Trace userId: string = '';
  @Trace requiresAccountAuth: boolean = false;
  @Trace accountUsername: string = '';
  @Trace accountUserId: string = '';
  @Trace authenticatedUserId: string = '';
  @Trace hostCapabilities: string[] = [];
  @Trace controlTargetType: string = 'none';
  @Trace controlTargetDeviceId: string = '';
  @Trace controlTargetDeviceName: string = '';
  @Trace connectionState: string = 'idle';
  @Trace connectionFailureKind: string = '';
  @Trace isLoadingHome: boolean = false;
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
  @Trace downloadingFilePath: string = '';
  @Trace downloadedFilePath: string = '';
  @Trace fileDownloadStatus: string = '';
  @Trace sessionQuery: string = '';
  @Trace sessionFilter: string = 'all';
  @Trace hasMoreSessions: boolean = false;
  @Trace isLoadingSessions: boolean = false;
  @Trace isLoadingConversation: boolean = false;
  @Trace pendingSessionId: string = '';
  @Trace isConversationDismissed: boolean = false;
  @Trace sessionErrorText: string = '';
  // Deliberately untraced: the memo behind visibleSessions(), not state anything
  // renders. Tracing it would make reading the list a write to the list.
  private visibleSessionsCache: RemoteSession[] = [];
  private visibleSessionsSource: RemoteSession[] | undefined = undefined;
  private visibleSessionsQuery: string = ' ';
  get activeSession(): SessionSummary { return this.conversation.activeSession; }
  get sessions(): RemoteSession[] { return this.conversation.sessions; }
  get persistedMessages(): ChatMessage[] { return this.conversation.persistedMessages; }
  get optimisticMessages(): ChatMessage[] { return this.conversation.optimisticMessages; }
  get activeTurnMessage(): ChatMessage { return this.conversation.activeTurnMessage; }
  get hasMoreMessages(): boolean { return this.conversation.hasMoreMessages; }
  get timelineItems(): ObservableChatTimelineItem[] { return this.conversation.timelineItems; }
  get timelineRevision(): number { return this.conversation.timelineRevision; }
  get isBusy(): boolean { return this.conversation.isBusy; }
  get modelCatalog(): RemoteModelCatalog { return this.conversation.modelCatalog; }
  get selectedModelId(): string { return this.conversation.selectedModelId; }
  get statusText(): string { return this.conversation.statusText; }
  get chatInput(): string { return this.conversation.chatInput; }
  get selectedImages(): SelectedImageAttachment[] { return this.conversation.selectedImages; }
  get isVoiceListening(): boolean { return this.conversation.isVoiceListening; }

  setQuery(query: string): void {
    this.sessionQuery = query;
  }

  setFilter(filter: string): void {
    this.sessionFilter = filter;
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

  setAccountUsername(accountUsername: string): void {
    this.accountUsername = accountUsername;
  }

  setAccountUserId(accountUserId: string): void {
    this.accountUserId = accountUserId;
  }

  setControlTarget(type: string, deviceId: string, deviceName: string): void {
    this.controlTargetType = type;
    this.controlTargetDeviceId = deviceId;
    this.controlTargetDeviceName = deviceName;
  }

  clearControlTarget(): void {
    this.setControlTarget('none', '', '');
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

  setHostCapabilities(capabilities: string[]): void {
    this.hostCapabilities = capabilities.slice();
  }

  supportsHostCapability(capability: string): boolean {
    return this.hostCapabilities.indexOf(capability) >= 0;
  }

  setStatusText(statusText: string): void {
    this.conversation.setStatusText(statusText);
  }

  setConnectionState(connectionState: string): void {
    this.connectionState = connectionState;
  }

  setConnectionFailureKind(connectionFailureKind: string): void {
    this.connectionFailureKind = connectionFailureKind;
  }

  setBusy(isBusy: boolean): void {
    this.conversation.setBusy(isBusy);
  }

  setLoadingHome(isLoadingHome: boolean): void {
    this.isLoadingHome = isLoadingHome;
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
    this.conversation.setSessions(sessions);
    this.hasMoreSessions = hasMore;
    this.sessionErrorText = '';
  }

  setActiveSession(session: SessionSummary): void {
    this.conversation.setActiveSession(session);
  }

  clearActiveSession(): void {
    this.conversation.clearActiveSession();
    this.isLoadingConversation = false;
    this.pendingSessionId = '';
    this.isConversationDismissed = false;
    this.setModelCatalog(RemoteUiState.emptyModelCatalog(), '');
  }

  setTimelineProjection(
    persistedMessages: ChatMessage[],
    optimisticMessages: ChatMessage[],
    activeTurnMessage: ChatMessage,
    hasMoreMessages: boolean,
    timelineItems: ChatTimelineItem[]
  ): void {
    this.conversation.setTimelineProjection(
      persistedMessages,
      optimisticMessages,
      activeTurnMessage,
      hasMoreMessages,
      timelineItems
    );
  }

  setHasMoreMessages(hasMoreMessages: boolean): void {
    this.conversation.setHasMoreMessages(hasMoreMessages);
  }

  clearTimeline(): void {
    this.conversation.clearTimeline();
  }

  setModelCatalog(modelCatalog: RemoteModelCatalog, selectedModelId: string): void {
    this.conversation.setModelCatalog(modelCatalog, selectedModelId);
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
    this.conversation.setChatInput(chatInput);
  }

  setSelectedImages(selectedImages: SelectedImageAttachment[]): void {
    this.conversation.setSelectedImages(selectedImages);
  }

  addSelectedImages(selectedImages: SelectedImageAttachment[]): void {
    this.conversation.addSelectedImages(selectedImages);
  }

  removeSelectedImage(imageId: string): void {
    this.conversation.removeSelectedImage(imageId);
  }

  clearComposer(): void {
    this.conversation.clearComposer();
  }

  setVoiceListening(isVoiceListening: boolean): void {
    this.conversation.setVoiceListening(isVoiceListening);
  }

  setLoading(loading: boolean): void {
    this.isLoadingSessions = loading;
  }

  setConversationLoading(loading: boolean): void {
    this.isLoadingConversation = loading;
  }

  setPendingSessionId(sessionId: string): void {
    this.pendingSessionId = sessionId;
  }

  setConversationDismissed(dismissed: boolean): void {
    this.isConversationDismissed = dismissed;
  }

  setError(errorText: string): void {
    this.sessionErrorText = errorText;
    this.isLoadingSessions = false;
  }

  clear(): void {
    this.conversation.setSessions([]);
    this.hasMoreSessions = false;
    this.isLoadingSessions = false;
    this.isLoadingConversation = false;
    this.pendingSessionId = '';
    this.isConversationDismissed = false;
    this.isLoadingHome = false;
    this.sessionErrorText = '';
    this.hostCapabilities = [];
  }

  /** Clears every conversation-scoped value when its remote identity ends. */
  resetConversationContext(): void {
    this.conversation.reset();
    this.hasMoreSessions = false;
    this.isLoadingSessions = false;
    this.isLoadingConversation = false;
    this.pendingSessionId = '';
    this.isConversationDismissed = false;
    this.isLoadingHome = false;
    this.sessionErrorText = '';
    this.hostCapabilities = [];
    this.clearDownloadStatus();
  }

  clearWorkspaceActions(): void {
    this.recentWorkspaces = [];
    this.assistants = [];
    this.closePickers();
  }

  /**
   * The sessions the sidebar may show, as a stable array.
   *
   * This is read straight into a `@Param`, so returning a fresh array each call
   * marked the session list dirty on every rebuild of its parent — including
   * the rebuild that a selection change causes, which is the one case where
   * nothing about the list has moved. Handing back the same instance until the
   * source or the query changes lets the list keep what it already computed.
   */
  visibleSessions(): RemoteSession[] {
    const source = this.conversation.sessions;
    const query = this.sessionQuery.trim().toLowerCase();
    if (source === this.visibleSessionsSource && query === this.visibleSessionsQuery) {
      return this.visibleSessionsCache;
    }
    this.visibleSessionsSource = source;
    this.visibleSessionsQuery = query;
    this.visibleSessionsCache = source.filter((item: RemoteSession) => {
      if (item.id.length === 0 || item.status === 'archived') {
        return false;
      }
      return query.length === 0 || item.title.toLowerCase().indexOf(query) >= 0;
    });
    return this.visibleSessionsCache;
  }

  hasRunningActiveTurn(): boolean {
    return this.conversation.hasRunningActiveTurn();
  }
}
