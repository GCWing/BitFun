import { ChatMessage, ChatMessageItemResponse, RemoteToolStatusResponse } from '../model/RemoteModels';

export type ChatTimelineItemType =
  | 'user_message'
  | 'assistant_message'
  | 'optimistic_user_message'
  | 'assistant_live_turn'
  | 'empty_state';

export interface ChatTimelineItem {
  id: string;
  type: ChatTimelineItemType;
  message?: ChatMessage;
  isStreaming: boolean;
  isFinalizing: boolean;
  showRetryAction?: boolean;
}

/** Shared render invalidation for both remote polling and general-chat streaming. */
export class ChatTimelineRevisionTracker {
  private signature: string = '';
  private revision: number = 0;

  /**
   * Identity of a single rendered item: its own id, plus everything about it
   * the bubble draws.
   *
   * Doubles as the timeline's `ForEach` key, which is the point of it being
   * per-item. Keying on the revision instead made every bubble in the
   * transcript a new node whenever any one of them changed — during a run that
   * is a full rebuild per poll tick, to show one message growing.
   */
  static itemSignature(item: ChatTimelineItem): string {
    const message = item.message;
    return `${item.type}:${item.id}:${item.isStreaming}:${item.isFinalizing}:` +
      `${message ? message.status : ''}:${message ? message.renderVersion || 0 : 0}:` +
      `${message ? ChatTimelineRevisionTracker.textSignature(message.text) : ''}:` +
      `${message && message.error ? ChatTimelineRevisionTracker.textSignature(message.error) : ''}:` +
      `${message && message.thinking ? ChatTimelineRevisionTracker.textSignature(message.thinking) : ''}:` +
      `${message && message.items ? message.items.length : 0}:${message && message.tools ? message.tools.length : 0}`;
  }

  update(items: ChatTimelineItem[]): number {
    const nextSignature = items
      .map((item: ChatTimelineItem) => ChatTimelineRevisionTracker.itemSignature(item))
      .join('|');
    if (nextSignature !== this.signature) {
      this.signature = nextSignature;
      this.revision += 1;
    }
    return this.revision;
  }

  private static textSignature(value: string): string {
    let hash = 0;
    for (let index = 0; index < value.length; index++) {
      hash = ((hash << 5) - hash + value.charCodeAt(index)) | 0;
    }
    return `${value.length}:${Math.abs(hash).toString(36)}`;
  }

  reset(): number {
    this.signature = '';
    this.revision += 1;
    return this.revision;
  }
}

export class ChatTimelineProjector {
  /**
   * `activeTurnAnchorId` is the id of the optimistic message the running turn
   * is answering. Anything sent after that message is queued behind the turn,
   * so it belongs below the reply rather than above it — appending the turn
   * last would push a mid-run message up under the previous one instead.
   */
  static project(
    messages: ChatMessage[],
    pendingMessages: ChatMessage[],
    activeTurn: ChatMessage,
    hasMoreMessages: boolean,
    activeTurnAnchorId: string = ''
  ): ChatTimelineItem[] {
    const timelineMessages = ChatTimelineProjector.realMessages(messages);
    const pendingItems = ChatTimelineProjector.pendingMessagesNotPersisted(pendingMessages, timelineMessages);
    const shouldRenderActiveTurn = ChatTimelineProjector.shouldRenderActiveTurn(timelineMessages, activeTurn);
    const anchorIndex = ChatTimelineProjector.anchorIndex(pendingItems, activeTurnAnchorId);
    const items: ChatTimelineItem[] = timelineMessages.map((message: ChatMessage) => {
      const item: ChatTimelineItem = {
        id: `message-${message.id}`,
        type: ChatTimelineProjector.messageItemType(message),
        message,
        isStreaming: false,
        isFinalizing: false,
        showRetryAction: false
      };
      return item;
    });

    // The message that started the turn has usually been persisted by the time
    // the reply streams, so no anchor is left to sit behind: whatever is still
    // optimistic was sent later and is waiting its turn.
    if (shouldRenderActiveTurn && anchorIndex < 0) {
      items.push(ChatTimelineProjector.activeTurnItem(activeTurn));
    }

    pendingItems.forEach((message: ChatMessage, index: number) => {
      const item: ChatTimelineItem = {
        id: `pending-${message.id}`,
        type: 'optimistic_user_message',
        message,
        isStreaming: false,
        isFinalizing: false,
        showRetryAction: false
      };
      items.push(item);
      if (shouldRenderActiveTurn && index === anchorIndex) {
        items.push(ChatTimelineProjector.activeTurnItem(activeTurn));
      }
    });

    if (items.length === 0 && !hasMoreMessages) {
      const item: ChatTimelineItem = {
        id: 'empty-state',
        type: 'empty_state',
        isStreaming: false,
        isFinalizing: false,
        showRetryAction: false
      };
      items.push(item);
    }

    ChatTimelineProjector.markLatestFailedMessageRetryable(items);
    return items;
  }

  private static anchorIndex(pendingItems: ChatMessage[], activeTurnAnchorId: string): number {
    if (activeTurnAnchorId.length === 0) {
      return -1;
    }
    for (let index = 0; index < pendingItems.length; index++) {
      if (pendingItems[index].id === activeTurnAnchorId) {
        return index;
      }
    }
    return -1;
  }

  private static activeTurnItem(activeTurn: ChatMessage): ChatTimelineItem {
    const turnKey = ChatTimelineProjector.activeTurnKey(activeTurn);
    const item: ChatTimelineItem = {
      id: `active-${turnKey}-${ChatTimelineProjector.activeTurnVersionKey(activeTurn)}`,
      type: 'assistant_live_turn',
      message: activeTurn,
      isStreaming: (activeTurn.status || '').toLowerCase() === 'active',
      isFinalizing: (activeTurn.status || '').toLowerCase() === 'completed',
      showRetryAction: false
    };
    return item;
  }

  private static markLatestFailedMessageRetryable(items: ChatTimelineItem[]): void {
    for (let index = items.length - 1; index >= 0; index--) {
      const message = items[index].message;
      if (!message) {
        continue;
      }
      const failed = (message.status || '').toLowerCase() === 'failed';
      items[index].showRetryAction = failed;
      if (failed && message.role === 'assistant') {
        const retryText = ChatTimelineProjector.nearestUserText(items, index);
        if (retryText.length > 0) {
          items[index].message = {
            id: message.id,
            role: message.role,
            text: message.text,
            status: message.status,
            renderVersion: message.renderVersion,
            turnId: message.turnId,
            detail: retryText,
            error: message.error,
            timestamp: message.timestamp,
            thinking: message.thinking,
            tools: message.tools,
            items: message.items,
            images: message.images
          };
        }
      }
      return;
    }
  }

  private static nearestUserText(items: ChatTimelineItem[], beforeIndex: number): string {
    for (let index = beforeIndex - 1; index >= 0; index--) {
      const message = items[index].message;
      if (message && message.role === 'user' && (message.text || '').trim().length > 0) {
        return message.text;
      }
    }
    return '';
  }

  static pendingMessagesNotPersisted(pendingMessages: ChatMessage[], messages: ChatMessage[]): ChatMessage[] {
    return pendingMessages.filter((pending: ChatMessage) => {
      return !messages.some((message: ChatMessage) => {
        return pending.role === 'user' && message.role === 'user' && pending.id === message.id;
      });
    });
  }

  private static realMessages(messages: ChatMessage[]): ChatMessage[] {
    return messages.filter((message: ChatMessage) => {
      return !ChatTimelineProjector.isSeedMessage(message);
    });
  }

  private static shouldRenderActiveTurn(messages: ChatMessage[], activeTurn: ChatMessage): boolean {
    if (!activeTurn || activeTurn.id.length === 0) {
      return false;
    }
    return !messages.some((message: ChatMessage) => {
      return ChatTimelineProjector.isPersistedAssistantDuplicate(activeTurn, message);
    });
  }

  private static isSeedMessage(message: ChatMessage): boolean {
    return message.id.indexOf('system-') === 0 && message.role === 'assistant';
  }

  private static messageItemType(message: ChatMessage): ChatTimelineItemType {
    return message.role === 'user' ? 'user_message' : 'assistant_message';
  }

  private static isPersistedAssistantDuplicate(activeTurn: ChatMessage, message: ChatMessage): boolean {
    if (message.role !== 'assistant') {
      return false;
    }
    if (!ChatTimelineProjector.hasDisplayableAssistantFinal(message)) {
      return false;
    }
    if (message.id === activeTurn.id) {
      return true;
    }
    if (activeTurn.turnId && activeTurn.turnId.length > 0 && message.id === `${activeTurn.turnId}_assistant`) {
      return true;
    }
    return activeTurn.turnId !== undefined && activeTurn.turnId.length > 0 &&
      message.turnId !== undefined && message.turnId === activeTurn.turnId;
  }

  private static hasDisplayableAssistantFinal(message: ChatMessage): boolean {
    if ((message.text || '').trim().length > 0 && (message.text || '').trim() !== (message.thinking || '').trim()) {
      return true;
    }
    return ChatTimelineProjector.lastTopLevelText(message.items || []).length > 0;
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

  private static activeTurnKey(activeTurn: ChatMessage): string {
    if (activeTurn.turnId && activeTurn.turnId.length > 0) {
      return activeTurn.turnId;
    }
    return activeTurn.id;
  }

  private static activeTurnVersionKey(activeTurn: ChatMessage): string {
    const signature = [
      activeTurn.status || '',
      `${activeTurn.renderVersion || 0}`,
      `${(activeTurn.text || '').length}:${ChatTimelineProjector.stableTextHash(activeTurn.text || '')}`,
      `${(activeTurn.error || '').length}:${ChatTimelineProjector.stableTextHash(activeTurn.error || '')}`,
      `${(activeTurn.thinking || '').length}:${ChatTimelineProjector.stableTextHash(activeTurn.thinking || '')}`,
      ChatTimelineProjector.itemSignature(activeTurn.items || []),
      ChatTimelineProjector.toolSignature(activeTurn.tools || [])
    ].join('|');
    return ChatTimelineProjector.stableTextHash(signature);
  }

  private static itemSignature(items: ChatMessageItemResponse[]): string {
    const parts: string[] = [];
    items.forEach((item: ChatMessageItemResponse, index: number) => {
      parts.push([
        `${index}`,
        item.type || '',
        item.is_subagent === true ? '1' : '0',
        `${(item.content || '').length}:${ChatTimelineProjector.stableTextHash(item.content || '')}`,
        item.tool ? ChatTimelineProjector.toolSignature([item.tool]) : '',
        item.subItems ? ChatTimelineProjector.itemSignature(item.subItems) : ''
      ].join(':'));
    });
    return parts.join(',');
  }

  private static toolSignature(tools: RemoteToolStatusResponse[]): string {
    const parts: string[] = [];
    tools.forEach((tool: RemoteToolStatusResponse, index: number) => {
      const preview = `${tool.input_preview || ''}|${tool.result_preview || ''}|${tool.error_preview || ''}`;
      parts.push([
        `${index}`,
        tool.id || '',
        tool.name || '',
        tool.status || '',
        `${tool.duration_ms || 0}`,
        `${preview.length}:${ChatTimelineProjector.stableTextHash(preview)}`
      ].join(':'));
    });
    return parts.join(',');
  }

  private static stableTextHash(text: string): string {
    let hash = 0;
    for (let index = 0; index < text.length; index++) {
      hash = ((hash << 5) - hash + text.charCodeAt(index)) | 0;
    }
    return Math.abs(hash).toString(36);
  }
}
