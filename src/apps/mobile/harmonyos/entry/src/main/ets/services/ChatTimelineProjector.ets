import { ChatMessage, ChatMessageItemResponse } from '../model/RemoteModels';
import { ChatTimelineItem, ChatTimelineItemType } from '../model/ChatTimelineModels';

export { ChatTimelineItem, ChatTimelineItemType, ChatTimelineRevisionTracker } from '../model/ChatTimelineModels';

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
    const persistedMessages = ChatTimelineProjector.realMessages(messages);
    const shouldRenderActiveTurn = ChatTimelineProjector.shouldRenderActiveTurn(persistedMessages, activeTurn);
    // A running turn can be delivered both as an active snapshot and as a
    // persisted assistant stub containing only thinking/tools. They represent
    // one visual row and share one stable turn key, so retaining both would
    // violate the uniqueness contract required by Repeat.virtualScroll().
    const timelineMessages = shouldRenderActiveTurn ? persistedMessages.filter((message: ChatMessage) => {
      return !ChatTimelineProjector.isMatchingAssistantTurn(activeTurn, message);
    }) : persistedMessages;
    const pendingItems = ChatTimelineProjector.pendingMessagesNotPersisted(pendingMessages, timelineMessages);
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
      id: `active-${turnKey}`,
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
    if (!ChatTimelineProjector.hasDisplayableAssistantFinal(message)) {
      return false;
    }
    return ChatTimelineProjector.isMatchingAssistantTurn(activeTurn, message);
  }

  private static isMatchingAssistantTurn(activeTurn: ChatMessage, message: ChatMessage): boolean {
    if (message.role !== 'assistant') {
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
        type !== 'user-steering' &&
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

}
