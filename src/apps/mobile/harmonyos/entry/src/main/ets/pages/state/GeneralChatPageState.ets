import { ChatMessage, RemoteSession, SelectedImageAttachment, SessionSummary } from '../../model/RemoteModels';
import { ChatTimelineItem, ChatTimelineRevisionTracker } from '../../services/ChatTimelineProjector';
import { GeneralChatServiceState } from '../../services/general-chat/GeneralChatServiceState';
import { RemoteUiState } from '../../services/RemoteUiState';

@ObservedV2
export class GeneralChatPageState {
  @Trace activeSession: SessionSummary = GeneralChatPageState.emptySession();
  @Trace sessions: RemoteSession[] = [];
  @Trace persistedMessages: ChatMessage[] = [];
  @Trace optimisticMessages: ChatMessage[] = [];
  @Trace activeTurnMessage: ChatMessage = RemoteUiState.emptyActiveTurn();
  @Trace hasMoreMessages: boolean = false;
  @Trace timelineItems: ChatTimelineItem[] = [];
  @Trace timelineRevision: number = 0;
  @Trace isBusy: boolean = false;
  @Trace serviceState: GeneralChatServiceState = GeneralChatServiceState.Unconfigured;
  @Trace apiUrl: string = '';
  @Trace modelName: string = '';
  @Trace hasApiKey: boolean = false;
  @Trace statusText: string = '';
  @Trace chatInput: string = '';
  @Trace selectedImages: SelectedImageAttachment[] = [];
  @Trace isVoiceListening: boolean = false;
  private readonly timelineRevisionTracker: ChatTimelineRevisionTracker = new ChatTimelineRevisionTracker();

  setActiveSession(session: SessionSummary): void {
    this.activeSession = {
      sessionId: session.sessionId,
      title: session.title,
      workspacePath: session.workspacePath,
      agentType: 'chat',
      initialTurnId: session.initialTurnId
    };
  }

  clearActiveSession(): void {
    this.activeSession = GeneralChatPageState.emptySession();
    this.clearTimeline();
  }

  setSessions(sessions: RemoteSession[]): void {
    this.sessions = sessions.slice();
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
      GeneralChatPageState.copyMessage(activeTurnMessage) :
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

  setBusy(isBusy: boolean): void {
    this.isBusy = isBusy;
  }

  setConfiguration(
    apiUrl: string,
    modelName: string,
    hasApiKey: boolean,
    serviceState: GeneralChatServiceState
  ): void {
    this.apiUrl = apiUrl;
    this.modelName = modelName;
    this.hasApiKey = hasApiKey;
    this.serviceState = serviceState;
  }

  setServiceState(serviceState: GeneralChatServiceState): void {
    this.serviceState = serviceState;
  }

  setStatus(statusText: string): void {
    this.statusText = statusText;
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

  recentSessions(): RemoteSession[] {
    const recent = this.sessions.slice();
    recent.sort((first: RemoteSession, second: RemoteSession) => {
      if ((first.pinned === true) !== (second.pinned === true)) {
        return first.pinned === true ? -1 : 1;
      }
      return this.sessionTimeValue(second.updatedAt) - this.sessionTimeValue(first.updatedAt);
    });
    return recent;
  }

  pinnedSessionId(): string {
    const pinned = this.sessions.find((session: RemoteSession) => session.pinned === true);
    return pinned ? pinned.id : '';
  }

  hasRunningActiveTurn(): boolean {
    return this.activeTurnMessage.id.length > 0 &&
      (this.activeTurnMessage.status || '').toLowerCase() === 'active';
  }

  latestUserMessageText(): string {
    const candidates = this.persistedMessages.concat(this.optimisticMessages);
    for (let index = candidates.length - 1; index >= 0; index--) {
      if (candidates[index].role === 'user' && candidates[index].text.trim().length > 0) {
        return candidates[index].text;
      }
    }
    return '';
  }

  private sessionTimeValue(value: string): number {
    const parsed = new Date(value).getTime();
    return Number.isNaN(parsed) ? 0 : parsed;
  }

  private static emptySession(): SessionSummary {
    return {
      sessionId: '',
      title: '',
      workspacePath: '',
      agentType: 'chat'
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
}
