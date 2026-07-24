import { ChatMessage, ImageAttachment, RemoteSession, SessionSummary } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from '../ConnectionErrorPolicy';
import { RemoteLogger } from '../RemoteLogger';
import { GeneralChatExportFormatter } from './GeneralChatExportFormatter';
import { GeneralChatSendResult, GeneralChatStreamObserver } from './GeneralChatPort';

export interface GeneralChatCommandClient {
  init(context: Context): Promise<void>;
  sessions(): RemoteSession[];
  createSession(firstMessage: string): Promise<RemoteSession>;
  messages(sessionId: string): Promise<ChatMessage[]>;
  sendMessage(
    sessionId: string,
    text: string,
    images: ImageAttachment[],
    observer?: GeneralChatStreamObserver
  ): Promise<GeneralChatSendResult>;
  cancelCurrentRequest(): void;
  recordAssistantMessage(sessionId: string, message: ChatMessage): Promise<void>;
  renameSession(sessionId: string, title: string): Promise<void>;
  archiveSession(sessionId: string, archived: boolean): Promise<void>;
  pinSession(sessionId: string, pinned: boolean): Promise<void>;
  deleteSession(sessionId: string): Promise<void>;
  removeFailedUserMessage(sessionId: string, text: string): Promise<void>;
}

export interface GeneralChatCommandCallbacks {
  onSessions: (sessions: RemoteSession[]) => void;
  onSessionPrepared: (sessionId: string) => void;
  onActiveSession: (session: SessionSummary) => void;
  onMessagesLoaded: (messages: ChatMessage[]) => void;
  onClearComposer: () => void;
  onChatInput: (text: string) => void;
  onStatusText: (statusText: string) => void;
  onBusy: (isBusy: boolean) => void;
  onToast: (statusText: string) => void;
}

export class GeneralChatCommandController {
  private readonly client: GeneralChatCommandClient;
  private readonly callbacks: GeneralChatCommandCallbacks;

  constructor(client: GeneralChatCommandClient, callbacks: GeneralChatCommandCallbacks) {
    this.client = client;
    this.callbacks = callbacks;
  }

  async init(context: Context): Promise<void> {
    await this.client.init(context);
  }

  async openSession(
    session: RemoteSession,
    isBusy: boolean,
    restoreDraft: (sessionId: string) => Promise<string>,
    onOpened: (sessionId: string) => void
  ): Promise<boolean> {
    if (!this.isValidChatSession(session) || isBusy) {
      return false;
    }
    try {
      this.callbacks.onBusy(true);
      this.callbacks.onSessionPrepared(session.id);
      this.callbacks.onActiveSession({
        sessionId: session.id,
        title: session.title,
        workspacePath: '',
        agentType: 'chat'
      });
      this.callbacks.onClearComposer();
      this.callbacks.onChatInput(await restoreDraft(session.id));
      this.callbacks.onMessagesLoaded(await this.client.messages(session.id));
      this.callbacks.onStatusText(RemoteI18n.t('generalChat.ready'));
      onOpened(session.id);
      return true;
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
      return false;
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async createSession(
    firstMessage: string,
    isBusy: boolean,
    clearHomeDraft: () => Promise<void>,
    onCreated: (sessionId: string) => void
  ): Promise<boolean> {
    const trimmed = firstMessage.trim();
    if (trimmed.length === 0 || isBusy) {
      return false;
    }
    try {
      this.callbacks.onBusy(true);
      const session = await this.client.createSession(trimmed);
      this.callbacks.onSessions(this.client.sessions());
      this.callbacks.onSessionPrepared(session.id);
      this.callbacks.onActiveSession({
        sessionId: session.id,
        title: session.title,
        workspacePath: '',
        agentType: 'chat'
      });
      onCreated(session.id);
      await clearHomeDraft();
      return true;
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
      return false;
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  refreshSessions(): void {
    this.callbacks.onSessions(this.client.sessions());
  }

  async loadMessages(sessionId: string): Promise<ChatMessage[]> {
    return this.client.messages(sessionId);
  }

  async sendMessage(
    sessionId: string,
    text: string,
    images: ImageAttachment[],
    observer?: GeneralChatStreamObserver
  ): Promise<GeneralChatSendResult> {
    return this.client.sendMessage(sessionId, text, images, observer);
  }

  cancelCurrentRequest(): void {
    this.client.cancelCurrentRequest();
  }

  async recordAssistantMessage(sessionId: string, message: ChatMessage): Promise<boolean> {
    if (sessionId.length === 0) {
      return false;
    }
    try {
      await this.client.recordAssistantMessage(sessionId, message);
      this.refreshSessions();
      return true;
    } catch (err) {
      RemoteLogger.warn(`general chat message persistence failed: ${ConnectionErrorPolicy.errorText(err)}`);
      return false;
    }
  }

  async deleteSession(session: RemoteSession, isBusy: boolean): Promise<void> {
    if (!this.isValidChatSession(session) || isBusy) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      this.callbacks.onStatusText(RemoteI18n.t('status.deletingSession'));
      await this.client.deleteSession(session.id);
      this.callbacks.onSessions(this.client.sessions());
      this.callbacks.onStatusText(RemoteI18n.t('status.sessionDeleted'));
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async archiveSession(session: RemoteSession, archived: boolean, isBusy: boolean): Promise<void> {
    if (!this.isValidChatSession(session) || isBusy) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      await this.client.archiveSession(session.id, archived);
      this.callbacks.onSessions(this.client.sessions());
      const statusText = RemoteI18n.t(archived ? 'status.sessionArchived' : 'status.sessionUnarchived');
      this.callbacks.onStatusText(statusText);
      this.callbacks.onToast(statusText);
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async pinSession(session: RemoteSession, pinned: boolean, isBusy: boolean): Promise<void> {
    if (!this.isValidChatSession(session) || isBusy) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      await this.client.pinSession(session.id, pinned);
      this.callbacks.onSessions(this.client.sessions());
      this.callbacks.onToast(pinned ? '已置顶会话' : '已取消置顶');
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async exportSession(
    session: RemoteSession,
    isBusy: boolean,
    onExportText: (text: string) => Promise<void>
  ): Promise<void> {
    if (!this.isValidChatSession(session) || isBusy) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      const messages = await this.client.messages(session.id);
      await onExportText(GeneralChatExportFormatter.markdown(session, messages));
      const statusText = RemoteI18n.t('status.sessionExported');
      this.callbacks.onStatusText(statusText);
      this.callbacks.onToast(statusText);
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async renameActiveSession(session: SessionSummary, title: string): Promise<boolean> {
    const sessionId = session.sessionId || '';
    const nextTitle = title.trim();
    if (sessionId.length === 0 || nextTitle.length === 0 || nextTitle === session.title) {
      return false;
    }
    try {
      await this.client.renameSession(sessionId, nextTitle);
      this.callbacks.onSessions(this.client.sessions());
      this.callbacks.onActiveSession({
        sessionId,
        title: nextTitle,
        workspacePath: '',
        agentType: 'chat',
        initialTurnId: session.initialTurnId
      });
      this.callbacks.onStatusText(RemoteI18n.t('status.titleUpdated'));
      return true;
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
      return false;
    }
  }

  async retryMessage(sessionId: string, text: string, isBusy: boolean): Promise<boolean> {
    if (isBusy || sessionId.length === 0) {
      return false;
    }
    try {
      await this.client.removeFailedUserMessage(sessionId, text);
      this.callbacks.onMessagesLoaded(await this.client.messages(sessionId));
      this.callbacks.onChatInput(text);
      return true;
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
      return false;
    }
  }

  private isValidChatSession(session: RemoteSession): boolean {
    return session.agentType === 'chat' && session.id.length > 0;
  }
}
