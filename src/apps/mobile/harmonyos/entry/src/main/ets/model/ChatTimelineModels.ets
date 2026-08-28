import { ChatMessage } from './RemoteModels';

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

/**
 * Observable row retained for as long as a stable Repeat key remains visible.
 *
 * Repeat.virtualScroll() does not observe mutations inside plain object array
 * entries. Keeping the projected DTO separate and reconciling it into this V2
 * row state lets streaming content update without putting mutable content in
 * the row key or rebuilding the whole timeline.
 */
@ObservedV2
export class ObservableChatTimelineItem implements ChatTimelineItem {
  @Trace id: string = '';
  @Trace type: ChatTimelineItemType = 'empty_state';
  @Trace message: ChatMessage | undefined = undefined;
  @Trace isStreaming: boolean = false;
  @Trace isFinalizing: boolean = false;
  @Trace showRetryAction: boolean = false;

  constructor(item: ChatTimelineItem) {
    this.updateFrom(item);
  }

  updateFrom(item: ChatTimelineItem): void {
    this.id = item.id;
    this.type = item.type;
    this.message = item.message ? ObservableChatTimelineItem.copyMessage(item.message) : undefined;
    this.isStreaming = item.isStreaming;
    this.isFinalizing = item.isFinalizing;
    this.showRetryAction = item.showRetryAction === true;
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
      error: message.error,
      timestamp: message.timestamp,
      thinking: message.thinking,
      tools: message.tools ? message.tools.slice() : undefined,
      items: message.items ? message.items.slice() : undefined,
      images: message.images ? message.images.slice() : undefined
    };
  }
}

/** Keyed bridge from pure timeline projections to ArkUI-observable rows. */
export class ChatTimelineRowStore {
  private rowsByKey: Map<string, ObservableChatTimelineItem> = new Map<string, ObservableChatTimelineItem>();
  private signaturesByKey: Map<string, string> = new Map<string, string>();

  reconcile(items: ChatTimelineItem[]): ObservableChatTimelineItem[] {
    // A lazy container requires unique keys. Keep the final projection for a
    // key as a defensive boundary even if an upstream adapter temporarily
    // exposes both a persisted assistant stub and its active turn snapshot.
    const orderedKeys: string[] = [];
    const itemsByKey: Map<string, ChatTimelineItem> = new Map<string, ChatTimelineItem>();
    items.forEach((item: ChatTimelineItem) => {
      const key = ChatTimelineRevisionTracker.itemKey(item);
      if (!itemsByKey.has(key)) {
        orderedKeys.push(key);
      }
      itemsByKey.set(key, item);
    });
    const nextRowsByKey: Map<string, ObservableChatTimelineItem> =
      new Map<string, ObservableChatTimelineItem>();
    const nextSignaturesByKey: Map<string, string> = new Map<string, string>();
    const rows: ObservableChatTimelineItem[] = [];
    orderedKeys.forEach((key: string) => {
      const item = itemsByKey.get(key);
      if (!item) {
        return;
      }
      const signature = ChatTimelineRevisionTracker.itemSignature(item);
      let row = this.rowsByKey.get(key);
      if (row) {
        if (this.signaturesByKey.get(key) !== signature) {
          row.updateFrom(item);
        }
      } else {
        row = new ObservableChatTimelineItem(item);
      }
      nextRowsByKey.set(key, row);
      nextSignaturesByKey.set(key, signature);
      rows.push(row);
    });
    this.rowsByKey = nextRowsByKey;
    this.signaturesByKey = nextSignaturesByKey;
    return rows;
  }

  clear(): void {
    this.rowsByKey.clear();
    this.signaturesByKey.clear();
  }
}

/** Shared render invalidation for both remote polling and general-chat streaming. */
export class ChatTimelineRevisionTracker {
  private signature: string = '';
  private revision: number = 0;

  /** Stable identity used by lazy UI containers. It must not include mutable content. */
  static itemKey(item: ChatTimelineItem): string {
    if (item.message && item.message.role === 'assistant' && (item.message.turnId || '').length > 0) {
      return `assistant_turn:${item.message.turnId}`;
    }
    return `${item.type}:${item.id}`;
  }

  /** Everything about one item that can alter its rendered presentation. */
  static itemSignature(item: ChatTimelineItem): string {
    const message = item.message;
    return `${ChatTimelineRevisionTracker.itemKey(item)}:${item.isStreaming}:${item.isFinalizing}:` +
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

  reset(): number {
    this.signature = '';
    this.revision += 1;
    return this.revision;
  }

  private static textSignature(value: string): string {
    let hash = 0;
    for (let index = 0; index < value.length; index++) {
      hash = ((hash << 5) - hash + value.charCodeAt(index)) | 0;
    }
    return `${value.length}:${Math.abs(hash).toString(36)}`;
  }
}
