import {
  ChatMessage,
  ImageAttachment,
  RemoteModelCatalog,
  RemoteModelConfig,
  RemoteSession
} from '../model/RemoteModels';
import { RemoteI18n } from '../i18n/RemoteI18n';

export class RemoteUiState {
  static canUseRemote(connectionState: string): boolean {
    return connectionState === 'connected' ||
      connectionState === 'reconnecting' ||
      connectionState === 'pairing';
  }

  static desktopNameFromRemoteUrl(value: string): string {
    return RemoteUiState.queryValueFromRemoteUrl(value, 'dn') || 'MacBook Pro';
  }

  static desktopIdFromRemoteUrl(value: string): string {
    return RemoteUiState.queryValueFromRemoteUrl(value, 'did');
  }

  private static queryValueFromRemoteUrl(value: string, key: string): string {
    const marker = `${key}=`;
    const index = value.indexOf(marker);
    if (index < 0) {
      return '';
    }
    const tail = value.slice(index + marker.length);
    const end = tail.indexOf('&');
    const encoded = end >= 0 ? tail.slice(0, end) : tail;
    try {
      return decodeURIComponent(encoded.replace(/\+/g, ' '));
    } catch (_err) {
      return encoded;
    }
  }

  static normalizeRemoteFilePath(path: string): string {
    return path
      .replace(/^computer:\/\//, '')
      .replace(/^file:\/\//, '')
      .trim();
  }

  static formatBytes(value: number): string {
    if (value < 1024) {
      return `${value} B`;
    }
    if (value < 1024 * 1024) {
      return `${Math.round(value / 1024)} KB`;
    }
    return `${Math.round(value / 1024 / 1024)} MB`;
  }

  static mergeMessages(current: ChatMessage[], incoming: ChatMessage[]): ChatMessage[] {
    const merged = current.slice();
    incoming.forEach((item: ChatMessage) => {
      const existingIndex = merged.findIndex((existing: ChatMessage) => {
        return existing.id === item.id;
      });
      if (existingIndex >= 0) {
        merged[existingIndex] = RemoteUiState.mergeMessageSnapshot(merged[existingIndex], item);
      } else {
        const optimisticIndex = merged.findIndex((existing: ChatMessage) => {
          return RemoteUiState.isOptimisticDuplicate(existing, item);
        });
        if (optimisticIndex >= 0) {
          merged[optimisticIndex] = item;
        } else {
          merged.push(item);
        }
      }
    });
    return merged;
  }

  static mergeRemoteWithLocalOptimistic(remoteMessages: ChatMessage[], currentMessages: ChatMessage[]): ChatMessage[] {
    const merged = remoteMessages.slice();
    currentMessages.forEach((item: ChatMessage) => {
      if (!RemoteUiState.isLocalOptimisticMessage(item)) {
        return;
      }
      const alreadySynced = merged.some((remote: ChatMessage) => {
        return RemoteUiState.isOptimisticDuplicate(item, remote);
      });
      if (!alreadySynced) {
        merged.push(item);
      }
    });
    return merged;
  }

  static mergeSessions(current: RemoteSession[], incoming: RemoteSession[]): RemoteSession[] {
    const seen = new Set<string>();
    current.forEach((item: RemoteSession) => {
      seen.add(item.id);
    });
    const merged = current.slice();
    incoming.forEach((item: RemoteSession) => {
      if (!seen.has(item.id)) {
        merged.push(item);
        seen.add(item.id);
      }
    });
    return merged;
  }

  static updateSessionTitle(items: RemoteSession[], sessionId: string, title: string): RemoteSession[] {
    return items.map((item: RemoteSession) => {
      if (item.id !== sessionId) {
        return item;
      }
      const updated: RemoteSession = {
        id: item.id,
        title,
        agentType: item.agentType,
        status: item.status,
        updatedAt: item.updatedAt,
        createdAt: item.createdAt,
        messageCount: item.messageCount,
        workspacePath: item.workspacePath,
        workspaceName: item.workspaceName
      };
      return updated;
    });
  }

  static markMessageFailed(messages: ChatMessage[], messageId: string): ChatMessage[] {
    return messages.map((item: ChatMessage) => {
      if (item.id !== messageId) {
        return item;
      }
      const failed: ChatMessage = {
        id: item.id,
        turnId: item.turnId,
        role: item.role,
        text: item.text,
        status: 'failed',
        detail: item.detail,
        timestamp: item.timestamp,
        thinking: item.thinking,
        tools: item.tools,
        items: item.items,
        images: item.images
      };
      return failed;
    });
  }

  static localUserMessage(text: string, images?: ImageAttachment[]): ChatMessage {
    return {
      id: `msg-${Date.now()}`,
      role: 'user',
      text,
      status: 'sent',
      images: images || []
    };
  }

  static emptyActiveTurn(): ChatMessage {
    return {
      id: '',
      role: 'assistant',
      text: '',
      status: '',
      detail: ''
    };
  }

  static emptyModelCatalog(): RemoteModelCatalog {
    return {
      version: 0,
      models: [],
      default_models: {}
    };
  }

  static selectedModelIdForCatalog(catalog: RemoteModelCatalog, currentSelectedModelId: string): string {
    const sessionModelId = catalog.session_model_id || '';
    if (sessionModelId.length > 0) {
      return sessionModelId;
    }
    if (currentSelectedModelId.length > 0) {
      const exists = catalog.models.some((model: RemoteModelConfig) => {
        return model.id === currentSelectedModelId && model.enabled;
      });
      if (exists) {
        return currentSelectedModelId;
      }
    }
    return catalog.default_models.primary || catalog.default_models.fast || currentSelectedModelId;
  }

  static seedMessage(title: string): ChatMessage {
    return {
      id: `system-${Date.now()}`,
      role: 'assistant',
      text: RemoteI18n.f('ui.seedConnected', title),
      status: 'ready'
    };
  }

  private static isLocalOptimisticMessage(item: ChatMessage): boolean {
    return item.role === 'user' && item.id.indexOf('msg-') === 0;
  }

  private static isOptimisticDuplicate(local: ChatMessage, remote: ChatMessage): boolean {
    return RemoteUiState.isLocalOptimisticMessage(local) &&
      local.role === remote.role &&
      local.text.trim() === remote.text.trim() &&
      local.text.trim().length > 0;
  }

  private static mergeMessageSnapshot(previous: ChatMessage, incoming: ChatMessage): ChatMessage {
    const hasIncomingText = (incoming.text || '').trim().length > 0;
    return {
      id: incoming.id,
      turnId: incoming.turnId || previous.turnId,
      role: incoming.role || previous.role,
      text: hasIncomingText ? incoming.text : previous.text,
      status: incoming.status || previous.status,
      detail: incoming.detail || previous.detail,
      timestamp: incoming.timestamp || previous.timestamp,
      thinking: incoming.thinking !== undefined ? incoming.thinking : (hasIncomingText ? undefined : previous.thinking),
      tools: incoming.tools !== undefined ? incoming.tools : previous.tools,
      items: incoming.items !== undefined ? incoming.items : previous.items,
      images: incoming.images !== undefined ? incoming.images : previous.images,
      renderVersion: incoming.renderVersion !== undefined ? incoming.renderVersion : previous.renderVersion
    };
  }
}
