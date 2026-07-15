import { ChatMessage, ChatMessageItemResponse, ImageAttachment, RemoteToolStatusResponse } from '../model/RemoteModels';

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
}

export class ChatTimelineProjector {
  static project(
    messages: ChatMessage[],
    pendingMessages: ChatMessage[],
    activeTurn: ChatMessage,
    hasMoreMessages: boolean
  ): ChatTimelineItem[] {
    const timelineMessages = ChatTimelineProjector.realMessages(messages);
    const pendingItems = ChatTimelineProjector.pendingMessagesNotPersisted(pendingMessages, timelineMessages);
    const shouldRenderActiveTurn = ChatTimelineProjector.shouldRenderActiveTurn(timelineMessages, activeTurn);
    const items: ChatTimelineItem[] = timelineMessages.map((message: ChatMessage) => {
      const item: ChatTimelineItem = {
        id: `message-${message.id}`,
        type: ChatTimelineProjector.messageItemType(message),
        message,
        isStreaming: false,
        isFinalizing: false
      };
      return item;
    });

    pendingItems.forEach((message: ChatMessage) => {
      const item: ChatTimelineItem = {
        id: `pending-${message.id}`,
        type: 'optimistic_user_message',
        message,
        isStreaming: false,
        isFinalizing: false
      };
      items.push(item);
    });

    if (shouldRenderActiveTurn) {
      const turnKey = ChatTimelineProjector.activeTurnKey(activeTurn);
      const item: ChatTimelineItem = {
        id: `active-${turnKey}-${ChatTimelineProjector.activeTurnVersionKey(activeTurn)}`,
        type: 'assistant_live_turn',
        message: activeTurn,
        isStreaming: (activeTurn.status || '').toLowerCase() === 'active',
        isFinalizing: (activeTurn.status || '').toLowerCase() === 'completed'
      };
      items.push(item);
    }

    if (items.length === 0 && !hasMoreMessages) {
      const item: ChatTimelineItem = {
        id: 'empty-state',
        type: 'empty_state',
        isStreaming: false,
        isFinalizing: false
      };
      items.push(item);
    }

    return items;
  }

  static pendingMessagesNotPersisted(pendingMessages: ChatMessage[], messages: ChatMessage[]): ChatMessage[] {
    return pendingMessages.filter((pending: ChatMessage) => {
      return !messages.some((message: ChatMessage) => {
        return ChatTimelineProjector.isPersistedUserDuplicate(pending, message);
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
    const activeText = (activeTurn.text || '').trim();
    const messageText = (message.text || '').trim();
    return activeText.length > 0 && activeText === messageText;
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
    return ChatTimelineProjector.imageSignature(pending.images || []) === ChatTimelineProjector.imageSignature(message.images || []);
  }

  private static imageSignature(images: ImageAttachment[]): string {
    return images.map((image: ImageAttachment) => {
      return `${image.name || ''}:${image.data_url || ''}`;
    }).sort().join('|');
  }

  private static stableTextHash(text: string): string {
    let hash = 0;
    for (let index = 0; index < text.length; index++) {
      hash = ((hash << 5) - hash + text.charCodeAt(index)) | 0;
    }
    return Math.abs(hash).toString(36);
  }
}
