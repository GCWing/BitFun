import { ChatMessage, ChatMessageItemResponse } from '../model/RemoteModels';

/**
 * Whether a persisted or polled transcript row still has a body the timeline
 * can show. A hollow assistant is a real id with no text, thinking, tools,
 * items, or images — the shape left behind when a slim poll stub overwrites a
 * live turn, or when a cache row was written before the desktop settled.
 */
export class TranscriptIntegrityPolicy {
  static hasDisplayableBody(message: ChatMessage): boolean {
    if ((message.text || '').trim().length > 0) {
      return true;
    }
    if ((message.thinking || '').trim().length > 0) {
      return true;
    }
    if (message.tools && message.tools.length > 0) {
      return true;
    }
    if (TranscriptIntegrityPolicy.hasRenderableItems(message.items)) {
      return true;
    }
    if (message.images && message.images.length > 0) {
      return true;
    }
    return false;
  }

  static isHollowAssistant(message: ChatMessage): boolean {
    return (message.role || '').toLowerCase() === 'assistant' &&
      !TranscriptIntegrityPolicy.hasDisplayableBody(message);
  }

  static hasHollowAssistants(messages: ChatMessage[]): boolean {
    return messages.some((message: ChatMessage) => {
      return TranscriptIntegrityPolicy.isHollowAssistant(message);
    });
  }

  static hasRenderableItems(items?: ChatMessageItemResponse[]): boolean {
    if (!items || items.length === 0) {
      return false;
    }
    return items.some((entry: ChatMessageItemResponse) => {
      if ((entry.content || '').trim().length > 0 || !!entry.tool) {
        return true;
      }
      return TranscriptIntegrityPolicy.hasRenderableItems(entry.subItems);
    });
  }

  static describeMessages(messages: ChatMessage[]): string {
    const assistants = messages.filter((message: ChatMessage) => {
      return (message.role || '').toLowerCase() === 'assistant';
    });
    let hollow = 0;
    const parts: string[] = [];
    assistants.forEach((message: ChatMessage) => {
      if (TranscriptIntegrityPolicy.isHollowAssistant(message)) {
        hollow += 1;
      }
      parts.push(
        `id=${TranscriptIntegrityPolicy.shortId(message.id)} ` +
          `text=${(message.text || '').length} ` +
          `thinking=${(message.thinking || '').length} ` +
          `tools=${message.tools ? message.tools.length : 0} ` +
          `items=${message.items ? message.items.length : 0}`
      );
    });
    const detail = parts.length > 0 ? ` ${parts.join(' ')}` : '';
    return `total=${messages.length} assistants=${assistants.length} hollow=${hollow}${detail}`;
  }

  private static shortId(value: string): string {
    if (value.length <= 10) {
      return value;
    }
    return `${value.slice(0, 6)}...${value.slice(-4)}`;
  }
}
