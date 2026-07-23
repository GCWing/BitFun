import {
  ChatMessage,
  ChatMessageItemResponse,
  ImageAttachment,
  RemoteModelCatalog,
  RemoteToolStatusResponse
} from '../model/RemoteModels';
import { ChatSessionCursor, ChatSessionSnapshot } from './ChatSessionController';
import { ChatTimelineItem, ChatTimelineProjector } from './ChatTimelineProjector';
import { RemoteUiState } from './RemoteUiState';

export type ChatSyncPhase =
  | 'idle'
  | 'loading'
  | 'sending'
  | 'streaming'
  | 'finalizing'
  | 'reconnecting'
  | 'error';

export interface ChatTimelineState {
  sessionId: string;
  persistedMessages: ChatMessage[];
  optimisticMessages: ChatMessage[];
  activeTurn?: ChatMessage;
  syncPhase: ChatSyncPhase;
  cursor: ChatSessionCursor;
  modelCatalog: RemoteModelCatalog;
  selectedModelId: string;
}

export class ChatTimelineStore {
  private state: ChatTimelineState = ChatTimelineStore.emptyState('');

  reset(sessionId: string = ''): void {
    this.state = ChatTimelineStore.emptyState(sessionId);
  }

  snapshot(): ChatTimelineState {
    return {
      sessionId: this.state.sessionId,
      persistedMessages: this.state.persistedMessages.slice(),
      optimisticMessages: this.state.optimisticMessages.slice(),
      activeTurn: this.state.activeTurn,
      syncPhase: this.state.syncPhase,
      cursor: {
        pollVersion: this.state.cursor.pollVersion,
        knownMessageCount: this.state.cursor.knownMessageCount,
        knownModelCatalogVersion: this.state.cursor.knownModelCatalogVersion
      },
      modelCatalog: this.state.modelCatalog,
      selectedModelId: this.state.selectedModelId
    };
  }

  setSyncPhase(syncPhase: ChatSyncPhase): void {
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages: this.state.persistedMessages,
      optimisticMessages: this.state.optimisticMessages,
      activeTurn: this.state.activeTurn,
      syncPhase,
      cursor: this.state.cursor,
      modelCatalog: this.state.modelCatalog,
      selectedModelId: this.state.selectedModelId
    };
  }

  setCursor(cursor: ChatSessionCursor): void {
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages: this.state.persistedMessages,
      optimisticMessages: this.state.optimisticMessages,
      activeTurn: this.state.activeTurn,
      syncPhase: this.state.syncPhase,
      cursor: {
        pollVersion: cursor.pollVersion,
        knownMessageCount: cursor.knownMessageCount,
        knownModelCatalogVersion: cursor.knownModelCatalogVersion
      },
      modelCatalog: this.state.modelCatalog,
      selectedModelId: this.state.selectedModelId
    };
  }

  setModelCatalog(modelCatalog: RemoteModelCatalog, selectedModelId: string): void {
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages: this.state.persistedMessages,
      optimisticMessages: this.state.optimisticMessages,
      activeTurn: this.state.activeTurn,
      syncPhase: this.state.syncPhase,
      cursor: {
        pollVersion: this.state.cursor.pollVersion,
        knownMessageCount: this.state.cursor.knownMessageCount,
        knownModelCatalogVersion: modelCatalog.version || 0
      },
      modelCatalog,
      selectedModelId
    };
  }

  setSelectedModelId(selectedModelId: string): void {
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages: this.state.persistedMessages,
      optimisticMessages: this.state.optimisticMessages,
      activeTurn: this.state.activeTurn,
      syncPhase: this.state.syncPhase,
      cursor: this.state.cursor,
      modelCatalog: this.state.modelCatalog,
      selectedModelId
    };
  }

  setPersistedMessages(messages: ChatMessage[]): void {
    const persistedMessages = ChatTimelineStore.realMessages(messages);
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages,
      optimisticMessages: ChatTimelineStore.optimisticMessagesNotPersisted(this.state.optimisticMessages, persistedMessages),
      activeTurn: this.coveredByPersistedMessage(this.state.activeTurn, persistedMessages) ? undefined : this.state.activeTurn,
      syncPhase: this.state.syncPhase,
      cursor: this.state.cursor,
      modelCatalog: this.state.modelCatalog,
      selectedModelId: this.state.selectedModelId
    };
  }

  mergePersistedMessages(messages: ChatMessage[]): void {
    const persistedMessages = RemoteUiState.mergeMessages(this.state.persistedMessages, ChatTimelineStore.realMessages(messages));
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages,
      optimisticMessages: ChatTimelineStore.optimisticMessagesNotPersisted(this.state.optimisticMessages, persistedMessages),
      activeTurn: this.coveredByPersistedMessage(this.state.activeTurn, persistedMessages) ? undefined : this.state.activeTurn,
      syncPhase: this.state.syncPhase,
      cursor: this.state.cursor,
      modelCatalog: this.state.modelCatalog,
      selectedModelId: this.state.selectedModelId
    };
  }

  appendOptimisticMessage(message: ChatMessage): void {
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages: this.state.persistedMessages,
      optimisticMessages: this.state.optimisticMessages.concat([message]),
      activeTurn: this.state.activeTurn,
      syncPhase: 'sending',
      cursor: this.state.cursor,
      modelCatalog: this.state.modelCatalog,
      selectedModelId: this.state.selectedModelId
    };
  }

  markOptimisticMessageFailed(messageId: string): void {
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages: this.state.persistedMessages,
      optimisticMessages: RemoteUiState.markMessageFailed(this.state.optimisticMessages, messageId),
      activeTurn: this.state.activeTurn,
      syncPhase: 'error',
      cursor: this.state.cursor,
      modelCatalog: this.state.modelCatalog,
      selectedModelId: this.state.selectedModelId
    };
  }

  setLocalActiveTurn(turnId: string): void {
    const normalizedTurnId = turnId.trim();
    if (normalizedTurnId.length === 0) {
      return;
    }
    const activeId = `active-${normalizedTurnId}`;
    if (this.state.activeTurn && this.state.activeTurn.id === activeId) {
      const existing = this.state.activeTurn;
      const activeTurn: ChatMessage = {
        id: existing.id,
        turnId: existing.turnId && existing.turnId.length > 0 ? existing.turnId : normalizedTurnId,
        role: existing.role,
        text: existing.text,
        status: existing.status || 'active',
        detail: existing.detail,
        timestamp: existing.timestamp,
        thinking: existing.thinking,
        tools: existing.tools,
        items: existing.items,
        images: existing.images
      };
      this.state = {
        sessionId: this.state.sessionId,
        persistedMessages: this.state.persistedMessages,
        optimisticMessages: this.state.optimisticMessages,
        activeTurn,
        syncPhase: ChatTimelineStore.phaseForActiveTurn(activeTurn, 'streaming'),
        cursor: this.state.cursor,
        modelCatalog: this.state.modelCatalog,
        selectedModelId: this.state.selectedModelId
      };
      return;
    }
    if (this.state.activeTurn && this.state.activeTurn.id.length > 0 &&
      this.state.activeTurn.id !== activeId &&
      !ChatTimelineStore.isLocalPendingActiveTurn(this.state.activeTurn)) {
      return;
    }
    const activeTurn: ChatMessage = {
      id: activeId,
      turnId: normalizedTurnId,
      role: 'assistant',
      text: '',
      status: 'active',
      detail: ''
    };
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages: this.state.persistedMessages,
      optimisticMessages: this.state.optimisticMessages,
      activeTurn,
      syncPhase: 'streaming',
      cursor: this.state.cursor,
      modelCatalog: this.state.modelCatalog,
      selectedModelId: this.state.selectedModelId
    };
  }

  setPendingActiveTurn(localId: string): string {
    const normalizedLocalId = localId.trim();
    if (normalizedLocalId.length === 0) {
      return '';
    }
    const activeId = `active-pending-${normalizedLocalId}`;
    if (this.state.activeTurn && this.state.activeTurn.id.length > 0 &&
      this.state.activeTurn.id !== activeId &&
      !ChatTimelineStore.isLocalPendingActiveTurn(this.state.activeTurn)) {
      return '';
    }
    const activeTurn: ChatMessage = {
      id: activeId,
      role: 'assistant',
      text: '',
      status: 'active',
      detail: ''
    };
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages: this.state.persistedMessages,
      optimisticMessages: this.state.optimisticMessages,
      activeTurn,
      syncPhase: 'streaming',
      cursor: this.state.cursor,
      modelCatalog: this.state.modelCatalog,
      selectedModelId: this.state.selectedModelId
    };
    return activeId;
  }

  clearPendingActiveTurn(activeId: string): void {
    if (!this.state.activeTurn || this.state.activeTurn.id !== activeId ||
      !ChatTimelineStore.isLocalPendingActiveTurn(this.state.activeTurn)) {
      return;
    }
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages: this.state.persistedMessages,
      optimisticMessages: this.state.optimisticMessages,
      activeTurn: undefined,
      syncPhase: 'idle',
      cursor: this.state.cursor,
      modelCatalog: this.state.modelCatalog,
      selectedModelId: this.state.selectedModelId
    };
  }

  setActiveTurn(activeTurn?: ChatMessage): void {
    const normalized = activeTurn && activeTurn.id.length > 0 ? activeTurn : undefined;
    const merged = ChatTimelineStore.activeTurnForUpdate(this.state.activeTurn, normalized);
    const nextActiveTurn = this.activeTurnForState(merged);
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages: this.state.persistedMessages,
      optimisticMessages: this.state.optimisticMessages,
      activeTurn: nextActiveTurn,
      syncPhase: ChatTimelineStore.phaseForActiveTurn(nextActiveTurn, this.state.syncPhase),
      cursor: this.state.cursor,
      modelCatalog: this.state.modelCatalog,
      selectedModelId: this.state.selectedModelId
    };
  }

  clearActiveTurn(): void {
    this.state = {
      sessionId: this.state.sessionId,
      persistedMessages: this.state.persistedMessages,
      optimisticMessages: this.state.optimisticMessages,
      activeTurn: undefined,
      syncPhase: 'idle',
      cursor: this.state.cursor,
      modelCatalog: this.state.modelCatalog,
      selectedModelId: this.state.selectedModelId
    };
  }

  applySnapshot(snapshot: ChatSessionSnapshot): void {
    this.setCursor(snapshot.cursor);
    if (snapshot.newMessages.length > 0) {
      this.mergePersistedMessages(snapshot.newMessages);
    }
    this.setActiveTurn(snapshot.activeTurn);
    if (snapshot.modelCatalog) {
      const selected = RemoteUiState.selectedModelIdForCatalog(snapshot.modelCatalog, this.state.selectedModelId);
      this.setModelCatalog(snapshot.modelCatalog, selected);
    }
  }

  activeTurnOrEmpty(): ChatMessage {
    return this.state.activeTurn || RemoteUiState.emptyActiveTurn();
  }

  project(hasMoreMessages: boolean): ChatTimelineItem[] {
    return ChatTimelineProjector.project(
      this.state.persistedMessages,
      this.state.optimisticMessages,
      this.activeTurnOrEmpty(),
      hasMoreMessages
    );
  }

  static optimisticMessagesNotPersisted(optimisticMessages: ChatMessage[], persistedMessages: ChatMessage[]): ChatMessage[] {
    return optimisticMessages.filter((pending: ChatMessage) => {
      return !persistedMessages.some((message: ChatMessage) => {
        return ChatTimelineStore.isPersistedUserDuplicate(pending, message);
      });
    });
  }

  static isActiveTurnCoveredByMessages(activeTurn: ChatMessage, messages: ChatMessage[]): boolean {
    if (activeTurn.id.length === 0) {
      return true;
    }
    return messages.some((message: ChatMessage) => {
      return ChatTimelineStore.isPersistedAssistantDuplicate(activeTurn, message);
    });
  }

  private coveredByPersistedMessage(activeTurn: ChatMessage | undefined, messages: ChatMessage[]): boolean {
    if (!activeTurn || activeTurn.id.length === 0) {
      return false;
    }
    return ChatTimelineStore.isActiveTurnCoveredByMessages(activeTurn, messages);
  }

  private activeTurnForState(activeTurn: ChatMessage | undefined): ChatMessage | undefined {
    if (activeTurn && activeTurn.id.length > 0) {
      return this.coveredByPersistedMessage(activeTurn, this.state.persistedMessages) ? undefined : activeTurn;
    }
    if (this.state.activeTurn && ChatTimelineStore.shouldHoldCompletedTurn(this.state.activeTurn) &&
      !this.coveredByPersistedMessage(this.state.activeTurn, this.state.persistedMessages)) {
      return this.state.activeTurn;
    }
    return undefined;
  }

  private static isLocalPendingActiveTurn(activeTurn: ChatMessage): boolean {
    return activeTurn.id.indexOf('active-pending-') === 0 && (!activeTurn.turnId || activeTurn.turnId.length === 0);
  }

  private static emptyState(sessionId: string): ChatTimelineState {
    return {
      sessionId,
      persistedMessages: [],
      optimisticMessages: [],
      syncPhase: 'idle',
      cursor: {
        pollVersion: 0,
        knownMessageCount: 0,
        knownModelCatalogVersion: 0
      },
      modelCatalog: RemoteUiState.emptyModelCatalog(),
      selectedModelId: ''
    };
  }

  private static realMessages(messages: ChatMessage[]): ChatMessage[] {
    return messages.filter((message: ChatMessage) => {
      return !(message.id.indexOf('system-') === 0 && message.role === 'assistant');
    });
  }

  private static phaseForActiveTurn(activeTurn: ChatMessage | undefined, fallback: ChatSyncPhase): ChatSyncPhase {
    if (!activeTurn || activeTurn.id.length === 0) {
      return fallback === 'sending' ? 'sending' : 'idle';
    }
    const status = (activeTurn.status || '').toLowerCase();
    if (status === 'active') {
      return 'streaming';
    }
    if (status === 'completed') {
      return 'finalizing';
    }
    return fallback;
  }

  private static activeTurnForUpdate(previous: ChatMessage | undefined, incoming: ChatMessage | undefined): ChatMessage | undefined {
    if (!incoming || incoming.id.length === 0) {
      return undefined;
    }
    if (previous && previous.id.length > 0 && ChatTimelineStore.sameActiveTurn(previous, incoming)) {
      return ChatTimelineStore.mergeActiveTurn(previous, incoming);
    }
    return incoming;
  }

  private static sameActiveTurn(previous: ChatMessage, incoming: ChatMessage): boolean {
    const previousTurnId = previous.turnId || '';
    const incomingTurnId = incoming.turnId || '';
    if (previousTurnId.length > 0 && incomingTurnId.length > 0 && previousTurnId === incomingTurnId) {
      return true;
    }
    return previous.id === incoming.id &&
      ChatTimelineStore.isActiveTurnId(previous.id) &&
      ChatTimelineStore.isActiveTurnId(incoming.id);
  }

  private static mergeActiveTurn(previous: ChatMessage, incoming: ChatMessage): ChatMessage {
    const text = ChatTimelineStore.monotonicText(previous.text || '', incoming.text || '');
    const thinking = ChatTimelineStore.monotonicText(previous.thinking || '', incoming.thinking || '');
    const items = ChatTimelineStore.mergeActiveItems(previous.items || [], incoming.items || []);
    const images = incoming.images && incoming.images.length > 0 ? incoming.images : previous.images;
    const tools = incoming.tools !== undefined ? incoming.tools : previous.tools;
    const activeTurn: ChatMessage = {
      id: incoming.id,
      turnId: incoming.turnId || previous.turnId,
      role: incoming.role || previous.role,
      text,
      status: incoming.status || previous.status,
      detail: incoming.detail,
      timestamp: incoming.timestamp || previous.timestamp,
      thinking: thinking.length > 0 ? thinking : undefined,
      tools,
      items,
      images,
      renderVersion: incoming.renderVersion
    };
    return activeTurn;
  }

  private static mergeActiveItems(
    previousItems: ChatMessageItemResponse[],
    incomingItems: ChatMessageItemResponse[]
  ): ChatMessageItemResponse[] {
    if (incomingItems.length === 0) {
      return previousItems;
    }
    const merged: ChatMessageItemResponse[] = [];
    incomingItems.forEach((incoming: ChatMessageItemResponse, index: number) => {
      const previous = previousItems[index];
      if (previous && ChatTimelineStore.sameActiveItem(previous, incoming)) {
        merged.push(ChatTimelineStore.mergeActiveItem(previous, incoming));
      } else {
        merged.push(incoming);
      }
    });
    for (let index = incomingItems.length; index < previousItems.length; index++) {
      merged.push(previousItems[index]);
    }
    return merged;
  }

  private static mergeActiveItem(
    previous: ChatMessageItemResponse,
    incoming: ChatMessageItemResponse
  ): ChatMessageItemResponse {
    const content = ChatTimelineStore.shouldMergeItemContent(previous, incoming)
      ? ChatTimelineStore.monotonicText(previous.content || '', incoming.content || '')
      : (incoming.content !== undefined ? incoming.content : previous.content);
    const subItems = ChatTimelineStore.mergeActiveItems(previous.subItems || [], incoming.subItems || []);
    const item: ChatMessageItemResponse = {
      type: incoming.type !== undefined ? incoming.type : previous.type,
      content,
      tool: incoming.tool !== undefined
        ? ChatTimelineStore.mergeTool(previous.tool, incoming.tool)
        : previous.tool,
      is_subagent: incoming.is_subagent !== undefined ? incoming.is_subagent : previous.is_subagent,
      subItems
    };
    return item;
  }

  private static sameActiveItem(previous: ChatMessageItemResponse, incoming: ChatMessageItemResponse): boolean {
    const previousType = (previous.type || '').toLowerCase();
    const incomingType = (incoming.type || '').toLowerCase();
    if (previousType !== incomingType) {
      return false;
    }
    const previousToolId = previous.tool ? (previous.tool.id || '') : '';
    const incomingToolId = incoming.tool ? (incoming.tool.id || '') : '';
    if (previousToolId.length > 0 || incomingToolId.length > 0) {
      return previousToolId === incomingToolId;
    }
    return true;
  }

  private static shouldMergeItemContent(previous: ChatMessageItemResponse, incoming: ChatMessageItemResponse): boolean {
    return ChatTimelineStore.isTextLikeItem(previous) && ChatTimelineStore.isTextLikeItem(incoming);
  }

  private static isTextLikeItem(item: ChatMessageItemResponse): boolean {
    const type = (item.type || '').toLowerCase();
    return type === 'text' || type === 'message' || type === 'thinking';
  }

  private static monotonicText(previous: string, incoming: string): string {
    if (incoming.length === 0) {
      return previous;
    }
    if (incoming.indexOf(previous) === 0) {
      return incoming;
    }
    if (previous.indexOf(incoming) === 0) {
      return previous;
    }
    if (incoming.length > previous.length && previous.length === 0) {
      return incoming;
    }
    return previous;
  }

  private static mergeTool(
    previous: RemoteToolStatusResponse | undefined,
    incoming: RemoteToolStatusResponse | undefined
  ): RemoteToolStatusResponse | undefined {
    if (!incoming) {
      return previous;
    }
    return incoming;
  }

  private static isActiveTurnId(id: string): boolean {
    return id.indexOf('active-') === 0;
  }

  private static isPersistedAssistantDuplicate(activeTurn: ChatMessage, message: ChatMessage): boolean {
    if (message.role !== 'assistant') {
      return false;
    }
    if (!ChatTimelineStore.hasDisplayableAssistantFinal(message)) {
      return false;
    }
    if (message.id === activeTurn.id) {
      return true;
    }
    if (activeTurn.turnId && activeTurn.turnId.length > 0 && message.id === `${activeTurn.turnId}_assistant`) {
      return true;
    }
    const activeText = (activeTurn.text || '').trim();
    const messageText = (message.text || '').trim();
    return activeText.length > 0 && activeText === messageText;
  }

  private static shouldHoldCompletedTurn(activeTurn: ChatMessage): boolean {
    const status = (activeTurn.status || '').toLowerCase();
    return (status === 'completed' || status === 'done' || status === 'success') &&
      ChatTimelineStore.hasDisplayableAssistantFinal(activeTurn);
  }

  private static hasDisplayableAssistantFinal(message: ChatMessage): boolean {
    if ((message.text || '').trim().length > 0 && (message.text || '').trim() !== (message.thinking || '').trim()) {
      return true;
    }
    return ChatTimelineStore.lastTopLevelText(message.items || []).length > 0;
  }

  private static lastTopLevelText(items: ChatMessageItemResponse[]): string {
    for (let index = items.length - 1; index >= 0; index--) {
      const item = items[index];
      const type = (item.type || '').toLowerCase();
      if ((item.content || '').trim().length > 0 &&
        !item.tool &&
        item.is_subagent !== true &&
        type !== 'thinking' &&
        type !== 'tool' &&
        type !== 'subagent' &&
        type !== 'agent') {
        return (item.content || '').trim();
      }
    }
    return '';
  }

  private static isPersistedUserDuplicate(pending: ChatMessage, message: ChatMessage): boolean {
    if (pending.role !== 'user' || message.role !== 'user') {
      return false;
    }
    if (pending.id === message.id) {
      return true;
    }
    const pendingText = (pending.text || '').trim();
    const messageText = (message.text || '').trim();
    if (pendingText.length === 0 || pendingText !== messageText) {
      return false;
    }
    return ChatTimelineStore.imageSignature(pending.images || []) === ChatTimelineStore.imageSignature(message.images || []);
  }

  private static imageSignature(images: ImageAttachment[]): string {
    return images.map((image: ImageAttachment) => {
      return `${image.name || ''}:${image.data_url || ''}`;
    }).sort().join('|');
  }
}
