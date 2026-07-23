import {
  ChatMessage,
  RemoteImageContext,
  SelectedImageAttachment,
  SessionMessagesResult,
  SessionSummary
} from '../model/RemoteModels';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from './ConnectionErrorPolicy';

export interface RemoteChatCommandClient {
  getSessionMessages(sessionId: string): Promise<SessionMessagesResult>;
  sendMessage(sessionId: string, text: string, agentType?: string): Promise<string>;
  sendMessageWithImages(
    sessionId: string,
    text: string,
    agentType: string,
    imageContexts: RemoteImageContext[]
  ): Promise<string>;
  cancelTask(sessionId: string, turnId?: string): Promise<void>;
  renameSession(sessionId: string, title: string): Promise<void>;
}

export interface RemoteChatCommandCallbacks {
  onMessagesLoaded: (messages: ChatMessage[], hasMoreMessages: boolean) => void;
  onMessageCountKnown: (pollVersion: number, knownMessageCount: number) => void;
  onSendSucceeded: (turnId: string, pendingActiveId: string) => void;
  onSendFailed: (
    rawText: string,
    images: SelectedImageAttachment[],
    localMessageId: string,
    pendingActiveId: string
  ) => void;
  onActiveSession: (session: SessionSummary) => void;
  onSessionTitleChanged: (sessionId: string, title: string) => void;
  onStatusText: (statusText: string) => void;
  onBusy: (isBusy: boolean) => void;
  onPollRequested: () => void;
}

export class RemoteChatCommandController {
  private readonly client: RemoteChatCommandClient;
  private readonly callbacks: RemoteChatCommandCallbacks;

  constructor(client: RemoteChatCommandClient, callbacks: RemoteChatCommandCallbacks) {
    this.client = client;
    this.callbacks = callbacks;
  }

  async loadMessages(
    sessionId: string,
    canApply: (sessionId: string) => boolean
  ): Promise<void> {
    if (sessionId.length === 0) {
      return;
    }
    try {
      this.callbacks.onStatusText(RemoteI18n.t('status.loadMessages'));
      const result = await this.client.getSessionMessages(sessionId);
      if (!canApply(sessionId)) {
        return;
      }
      this.callbacks.onMessagesLoaded(result.messages, false);
      this.callbacks.onMessageCountKnown(0, result.messages.length);
      this.callbacks.onStatusText(RemoteI18n.t('status.messagesSynced'));
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
      this.callbacks.onMessageCountKnown(0, 0);
    }
  }

  async loadOlderMessages(
    sessionId: string,
    currentPollVersion: number,
    hasMoreMessages: boolean,
    isBusy: boolean
  ): Promise<void> {
    if (sessionId.length === 0 || !hasMoreMessages || isBusy) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      this.callbacks.onStatusText(RemoteI18n.t('status.loadOlderMessages'));
      const result = await this.client.getSessionMessages(sessionId);
      this.callbacks.onMessagesLoaded(result.messages, false);
      this.callbacks.onMessageCountKnown(currentPollVersion, result.messages.length);
      this.callbacks.onStatusText(RemoteI18n.t('status.messagesSynced'));
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async sendPreparedMessage(
    sessionId: string,
    text: string,
    agentType: string,
    rawText: string,
    images: SelectedImageAttachment[],
    imageContexts: RemoteImageContext[],
    localMessageId: string,
    pendingActiveId: string,
    isBusy: boolean,
    remoteAvailable: boolean
  ): Promise<void> {
    if ((text.length === 0 && images.length === 0) || sessionId.length === 0 || isBusy || !remoteAvailable) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      this.callbacks.onStatusText(images.length > 0 ? RemoteI18n.t('status.sendImage') : RemoteI18n.t('status.sendCommand'));
      const turnId = images.length > 0 ?
        await this.client.sendMessageWithImages(sessionId, text, agentType, imageContexts) :
        await this.client.sendMessage(sessionId, text, agentType);
      this.callbacks.onSendSucceeded(turnId, pendingActiveId);
      this.callbacks.onStatusText(RemoteI18n.t('status.sentWaiting'));
    } catch (err) {
      this.callbacks.onSendFailed(rawText, images, localMessageId, pendingActiveId);
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async stopTask(
    sessionId: string,
    activeTurnMessageId: string,
    activeTurnId: string,
    remoteAvailable: boolean
  ): Promise<void> {
    if (sessionId.length === 0 || !remoteAvailable) {
      return;
    }
    if (activeTurnMessageId.length === 0) {
      this.callbacks.onStatusText(RemoteI18n.t('status.noRunningTask'));
      return;
    }
    try {
      this.callbacks.onStatusText(RemoteI18n.t('status.stoppingTask'));
      await this.client.cancelTask(sessionId, activeTurnId);
      this.callbacks.onStatusText(RemoteI18n.t('status.stopRequested'));
      this.callbacks.onPollRequested();
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    }
  }

  async renameActiveSession(
    session: SessionSummary,
    title: string,
    isBusy: boolean,
    remoteAvailable: boolean
  ): Promise<void> {
    const sessionId = session.sessionId || '';
    const nextTitle = title.trim();
    if (sessionId.length === 0 || nextTitle.length === 0 || nextTitle === session.title || isBusy || !remoteAvailable) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      this.callbacks.onStatusText(RemoteI18n.t('status.updatingTitle'));
      await this.client.renameSession(sessionId, nextTitle);
      const updatedSession: SessionSummary = {
        sessionId,
        title: nextTitle,
        workspacePath: session.workspacePath,
        agentType: session.agentType,
        initialTurnId: session.initialTurnId
      };
      this.callbacks.onActiveSession(updatedSession);
      this.callbacks.onSessionTitleChanged(sessionId, nextTitle);
      this.callbacks.onStatusText(RemoteI18n.t('status.titleUpdated'));
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.callbacks.onBusy(false);
    }
  }
}
