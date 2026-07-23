import { ChatMessage, ImageAttachment, RemoteSession } from '../../model/RemoteModels';
import {
  GeneralChatLocalStore,
  GeneralChatPort,
  GeneralChatSendResult,
  GeneralChatStreamCallbacks,
  GeneralChatStreamObserver
} from './GeneralChatPort';

export class GeneralChatRepository {
  private sessions: RemoteSession[] = [];
  private messagesBySession: Map<string, ChatMessage[]> = new Map<string, ChatMessage[]>();
  private readonly port: GeneralChatPort;
  private readonly localStore: GeneralChatLocalStore;
  private requestCancelled: boolean = false;

  constructor(port: GeneralChatPort, localStore: GeneralChatLocalStore) {
    this.port = port;
    this.localStore = localStore;
  }

  async init(context: Context): Promise<void> {
    await this.localStore.init(context);
    await this.restoreFromLocal();
  }

  async restoreFromLocal(): Promise<void> {
    this.sessions = await this.localStore.listSessions();
    this.sortSessions();
  }

  listSessions(): RemoteSession[] {
    return this.sessions.slice();
  }

  async createSession(firstMessage: string): Promise<RemoteSession> {
    const session = await this.port.createSession(firstMessage);
    this.sessions = [session].concat(this.sessions.filter((item: RemoteSession) => item.id !== session.id));
    this.messagesBySession.set(session.id, []);
    await this.localStore.upsertSession(session);
    return session;
  }

  async messages(sessionId: string): Promise<ChatMessage[]> {
    const cached = this.messagesBySession.get(sessionId);
    if (cached) {
      return cached.slice();
    }
    const stored = await this.localStore.loadMessages(sessionId);
    this.messagesBySession.set(sessionId, stored);
    return stored.slice();
  }

  async sendMessage(
    sessionId: string,
    text: string,
    images: ImageAttachment[] = [],
    observer?: GeneralChatStreamObserver
  ): Promise<GeneralChatSendResult> {
    const existingMessages = await this.messages(sessionId);
    let didPersistUserMessage = false;
    let didReceiveDelta = false;
    let persistedUserMessage: ChatMessage | undefined = undefined;
    this.requestCancelled = false;
    const repositoryObserver = new GeneralChatStreamCallbacks(
      async (message: ChatMessage): Promise<void> => {
        persistedUserMessage = message;
        const messages = existingMessages.concat([message]);
        this.messagesBySession.set(sessionId, messages);
        await this.localStore.replaceMessages(sessionId, messages);
        await this.bumpSession(sessionId, messages.length);
        didPersistUserMessage = true;
        if (observer) {
          await observer.onUserMessage(message);
        }
      },
      (delta: string): void => {
        if (delta.length > 0) {
          didReceiveDelta = true;
        }
        if (observer) {
          observer.onDelta(delta);
        }
      }
    );
    try {
      const result = await this.port.sendMessage(sessionId, text, images, existingMessages, repositoryObserver);
      const sentUserMessage = GeneralChatRepository.messageWithStatus(result.userMessage, 'sent');
      if (!didPersistUserMessage) {
        const messages = existingMessages.concat([sentUserMessage]);
        this.messagesBySession.set(sessionId, messages);
        await this.localStore.replaceMessages(sessionId, messages);
        await this.bumpSession(sessionId, messages.length);
      } else {
        await this.replaceMessage(sessionId, sentUserMessage);
      }
      return {
        userMessage: sentUserMessage,
        assistantMessage: result.assistantMessage,
        streamChunks: result.streamChunks,
        tools: result.tools
      };
    } catch (err) {
      if (persistedUserMessage) {
        const status = this.requestCancelled || didReceiveDelta ? 'sent' : 'failed';
        await this.replaceMessage(sessionId, GeneralChatRepository.messageWithStatus(persistedUserMessage, status));
      }
      const message = err instanceof Error ? err.message : `${err}`;
      throw new Error(message);
    } finally {
      this.requestCancelled = false;
    }
  }

  cancelCurrentRequest(): void {
    this.requestCancelled = true;
    this.port.cancelCurrentRequest();
  }

  async recordAssistantMessage(sessionId: string, message: ChatMessage): Promise<void> {
    const messages = (await this.messages(sessionId))
      .filter((item: ChatMessage) => item.id !== message.id)
      .concat([message]);
    this.messagesBySession.set(sessionId, messages);
    await this.localStore.replaceMessages(sessionId, messages);
    await this.bumpSession(sessionId, messages.length);
    await this.generateFirstTurnTitle(sessionId, messages);
  }

  private async generateFirstTurnTitle(sessionId: string, messages: ChatMessage[]): Promise<void> {
    const session = this.sessions.find((item: RemoteSession) => item.id === sessionId);
    if (!session || session.titleGenerated === true) {
      return;
    }
    const userMessages = messages.filter((item: ChatMessage) => item.role === 'user');
    const assistantMessages = messages.filter((item: ChatMessage) => item.role === 'assistant');
    if (userMessages.length !== 1 || assistantMessages.length !== 1) {
      return;
    }
    try {
      const title = await this.port.generateTitle(userMessages[0].text, assistantMessages[0].text);
      if (title.trim().length === 0) {
        return;
      }
      const updated: RemoteSession = {
        id: session.id,
        title: title.trim(),
        agentType: session.agentType,
        status: session.status,
        updatedAt: session.updatedAt,
        createdAt: session.createdAt,
        messageCount: session.messageCount,
        workspacePath: session.workspacePath,
        workspaceName: session.workspaceName,
        pinned: session.pinned,
        titleGenerated: true
      };
      this.replaceSession(updated);
      await this.localStore.upsertSession(updated);
    } catch (_err) {
    }
  }

  async renameSession(sessionId: string, title: string): Promise<void> {
    const nextTitle = title.trim();
    if (nextTitle.length === 0) {
      return;
    }
    const session = this.sessions.find((item: RemoteSession) => item.id === sessionId);
    if (!session) {
      return;
    }
    const updated: RemoteSession = {
      id: session.id,
      title: nextTitle,
      agentType: session.agentType,
      status: session.status,
      updatedAt: new Date().toISOString(),
      createdAt: session.createdAt,
      messageCount: session.messageCount,
      workspacePath: session.workspacePath,
      workspaceName: session.workspaceName,
      pinned: session.pinned,
      titleGenerated: session.titleGenerated
    };
    this.replaceSession(updated);
    await this.localStore.upsertSession(updated);
  }

  async archiveSession(sessionId: string, archived: boolean): Promise<void> {
    const session = this.sessions.find((item: RemoteSession) => item.id === sessionId);
    if (!session) {
      return;
    }
    const updated: RemoteSession = {
      id: session.id,
      title: session.title,
      agentType: session.agentType,
      status: archived ? 'archived' : 'ready',
      updatedAt: new Date().toISOString(),
      createdAt: session.createdAt,
      messageCount: session.messageCount,
      workspacePath: session.workspacePath,
      workspaceName: session.workspaceName,
      pinned: session.pinned,
      titleGenerated: session.titleGenerated
    };
    this.replaceSession(updated);
    await this.localStore.upsertSession(updated);
  }

  async pinSession(sessionId: string, pinned: boolean): Promise<void> {
    for (let index = 0; index < this.sessions.length; index++) {
      const session = this.sessions[index];
      const nextPinned = session.id === sessionId ? pinned : false;
      if ((session.pinned === true) === nextPinned) {
        continue;
      }
      const updated: RemoteSession = {
        id: session.id,
        title: session.title,
        agentType: session.agentType,
        status: session.status,
        updatedAt: session.updatedAt,
        createdAt: session.createdAt,
        messageCount: session.messageCount,
        workspacePath: session.workspacePath,
        workspaceName: session.workspaceName,
        pinned: nextPinned,
        titleGenerated: session.titleGenerated
      };
      this.sessions[index] = updated;
      await this.localStore.upsertSession(updated);
    }
  }

  async deleteSession(sessionId: string): Promise<void> {
    this.sessions = this.sessions.filter((item: RemoteSession) => item.id !== sessionId);
    this.messagesBySession.delete(sessionId);
    await this.localStore.deleteSession(sessionId);
  }

  async draft(draftId: string): Promise<string> {
    return this.localStore.loadDraft(draftId);
  }

  async saveDraft(draftId: string, text: string): Promise<void> {
    if (text.length === 0) {
      await this.localStore.deleteDraft(draftId);
      return;
    }
    await this.localStore.saveDraft(draftId, text);
  }

  async clearDraft(draftId: string): Promise<void> {
    await this.localStore.deleteDraft(draftId);
  }

  async removeFailedUserMessage(sessionId: string, text: string): Promise<void> {
    const normalizedText = text.trim();
    const messages = (await this.messages(sessionId)).filter((message: ChatMessage) => {
      return !(message.role === 'user' && message.status === 'failed' &&
        message.text.trim() === normalizedText);
    });
    this.messagesBySession.set(sessionId, messages);
    await this.localStore.replaceMessages(sessionId, messages);
    await this.bumpSession(sessionId, messages.length);
  }

  private async replaceMessage(sessionId: string, message: ChatMessage): Promise<void> {
    const messages = (await this.messages(sessionId)).map((item: ChatMessage) => {
      return item.id === message.id ? message : item;
    });
    this.messagesBySession.set(sessionId, messages);
    await this.localStore.replaceMessages(sessionId, messages);
  }

  private async bumpSession(sessionId: string, messageCount: number): Promise<void> {
    const session = this.sessions.find((item: RemoteSession) => item.id === sessionId);
    if (!session) {
      return;
    }
    const updated: RemoteSession = {
      id: session.id,
      title: session.title,
      agentType: session.agentType,
      status: 'ready',
      updatedAt: new Date().toISOString(),
      createdAt: session.createdAt,
      messageCount,
      workspacePath: session.workspacePath,
      workspaceName: session.workspaceName,
      pinned: session.pinned,
      titleGenerated: session.titleGenerated
    };
    this.replaceSession(updated);
    await this.localStore.upsertSession(updated);
  }

  private replaceSession(session: RemoteSession): void {
    this.sessions = this.sessions.map((item: RemoteSession) => item.id === session.id ? session : item);
    this.sortSessions();
  }

  private sortSessions(): void {
    this.sessions.sort((first: RemoteSession, second: RemoteSession) => {
      return GeneralChatRepository.timeValue(second.updatedAt) - GeneralChatRepository.timeValue(first.updatedAt);
    });
  }

  private static timeValue(value: string): number {
    const parsed = new Date(value).getTime();
    return Number.isNaN(parsed) ? 0 : parsed;
  }

  private static messageWithStatus(message: ChatMessage, status: string): ChatMessage {
    return {
      id: message.id,
      role: message.role,
      text: message.text,
      status,
      renderVersion: message.renderVersion,
      turnId: message.turnId,
      detail: message.detail,
      timestamp: message.timestamp,
      thinking: message.thinking,
      tools: message.tools,
      items: message.items,
      images: message.images
    };
  }
}
