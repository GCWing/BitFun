import {
  ChatMessage,
  RemoteModelCatalog,
  RemoteSession,
  SelectedImageAttachment,
  SessionSummary
} from '../../model/RemoteModels';
import { ChatTimelineItem, ObservableChatTimelineItem } from '../../model/ChatTimelineModels';
import { GeneralChatServiceState } from '../../services/general-chat/GeneralChatServiceState';
import { ConversationCoreState } from './ConversationCoreState';

@ObservedV2
export class GeneralChatPageState {
  @Trace conversation: ConversationCoreState = new ConversationCoreState('chat');
  @Trace serviceState: GeneralChatServiceState = GeneralChatServiceState.Unconfigured;
  @Trace apiUrl: string = '';
  @Trace modelName: string = '';
  @Trace hasApiKey: boolean = false;

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

  setActiveSession(session: SessionSummary): void {
    this.conversation.setActiveSession(session);
  }

  clearActiveSession(): void {
    this.conversation.clearActiveSession();
  }

  setSessions(sessions: RemoteSession[]): void {
    this.conversation.setSessions(sessions);
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

  clearTimeline(): void {
    this.conversation.clearTimeline();
  }

  setBusy(isBusy: boolean): void {
    this.conversation.setBusy(isBusy);
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

  setModelCatalog(modelCatalog: RemoteModelCatalog, selectedModelId: string): void {
    this.conversation.setModelCatalog(modelCatalog, selectedModelId);
  }

  setStatus(statusText: string): void {
    this.conversation.setStatusText(statusText);
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

  recentSessions(): RemoteSession[] {
    const recent = this.conversation.sessions.slice();
    recent.sort((first: RemoteSession, second: RemoteSession) => {
      if ((first.pinned === true) !== (second.pinned === true)) {
        return first.pinned === true ? -1 : 1;
      }
      return this.sessionTimeValue(second.updatedAt) - this.sessionTimeValue(first.updatedAt);
    });
    return recent;
  }

  pinnedSessionId(): string {
    const pinned = this.conversation.sessions.find((session: RemoteSession) => session.pinned === true);
    return pinned ? pinned.id : '';
  }

  hasRunningActiveTurn(): boolean {
    return this.conversation.hasRunningActiveTurn();
  }

  latestUserMessageText(): string {
    return this.conversation.latestUserMessageText();
  }

  private sessionTimeValue(value: string): number {
    const parsed = new Date(value).getTime();
    return Number.isNaN(parsed) ? 0 : parsed;
  }
}
