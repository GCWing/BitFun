import {
  ChatMessage,
  ChatMessageItemResponse,
  RemoteToolStatusResponse,
  SelectedImageAttachment
} from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from '../../services/ConnectionErrorPolicy';
import { Encoding } from '../../services/Encoding';
import { ConversationViewModel } from './ConversationViewModel';
import { GeneralChatPageState } from '../state/GeneralChatPageState';
import { GeneralChatCommandController } from '../../services/general-chat/GeneralChatCommandController';
import { GeneralChatDraftLifecycleController } from '../../services/general-chat/GeneralChatDraftLifecycleController';
import { GeneralChatStreamLifecycleController } from '../../services/general-chat/GeneralChatStreamLifecycleController';
import {
  GeneralChatServiceState,
  GeneralChatServiceStatus
} from '../../services/general-chat/GeneralChatServiceState';
import {
  GeneralChatSendResult,
  GeneralChatStreamCallbacks
} from '../../services/general-chat/GeneralChatPort';
import { RemoteLogger } from '../../services/RemoteLogger';

export interface GeneralChatConversationViewModelHooks {
  readonly isVisible: (sessionId: string) => boolean;
  readonly currentActiveTurnId: () => string;
  readonly latestUserMessageText: () => string;
  readonly syncTimeline: () => void;
  readonly refreshSessions: () => void;
  readonly onTaskStarted?: (sessionId: string, turnId: string) => void;
  readonly onTaskCompleted?: (sessionId: string, turnId: string) => void;
  readonly onTaskStopped?: (sessionId: string, turnId: string) => void;
}

/** Owns General Chat stream state and publishes all updates through ConversationViewModel. */
export class GeneralChatConversationViewModel {
  private readonly pageState: GeneralChatPageState;
  private readonly command: GeneralChatCommandController;
  private readonly drafts: GeneralChatDraftLifecycleController;
  private readonly stream: GeneralChatStreamLifecycleController;
  private readonly timeline: ConversationViewModel;
  private readonly hooks: GeneralChatConversationViewModelHooks;

  constructor(
    pageState: GeneralChatPageState,
    command: GeneralChatCommandController,
    drafts: GeneralChatDraftLifecycleController,
    stream: GeneralChatStreamLifecycleController,
    timeline: ConversationViewModel,
    hooks: GeneralChatConversationViewModelHooks
  ) {
    this.pageState = pageState;
    this.command = command;
    this.drafts = drafts;
    this.stream = stream;
    this.timeline = timeline;
    this.hooks = hooks;
  }

  async sendMessage(): Promise<void> {
    const rawText = this.pageState.chatInput.trim();
    const images = this.pageState.selectedImages.slice();
    const text = rawText.length > 0 ? rawText :
      (images.length > 0 ? RemoteI18n.t('chat.analyzeImageDefault') : '');
    const sessionId = this.pageState.activeSession.sessionId || '';
    if ((!text && images.length === 0) || !sessionId || this.pageState.isBusy) {
      return;
    }
    this.pageState.clearComposer();
    this.drafts.clear(sessionId);
    const localMessage = this.localUserMessage(text, images);
    this.timeline.appendOptimisticMessage(localMessage);
    this.hooks.syncTimeline();
    try {
      this.pageState.setBusy(true);
      this.pageState.setServiceState(GeneralChatServiceState.Sending);
      this.pageState.setStatus(RemoteI18n.t('generalChat.generating'));
      this.beginStream(sessionId);
      const observer = new GeneralChatStreamCallbacks(
        async (message: ChatMessage): Promise<void> => {
          this.timeline.dispatch({ type: 'user_message', message, persisted: true });
          this.hooks.syncTimeline();
        },
        (delta: string): void => {
          this.appendDelta(sessionId, delta);
        }
      );
      this.stream.setRequestInFlight(true);
      const result: GeneralChatSendResult = await this.command.sendMessage(
        sessionId,
        text,
        images,
        observer
      );
      this.stream.setRequestInFlight(false);
      this.flushDelta(sessionId);
      this.hooks.refreshSessions();
      if (this.stream.text().length > 0) {
        await this.finishStream(sessionId, result);
      } else {
        const finalMessage = this.messageForActiveTurn(result.assistantMessage);
        this.resetStream();
        this.timeline.dispatch({ type: 'user_message', message: result.userMessage, persisted: true });
        this.timeline.dispatch({
          type: 'turn_finished',
          sessionId,
          turnId: finalMessage.turnId || this.hooks.currentActiveTurnId(),
          message: finalMessage
        });
        await this.persistAssistantMessage(sessionId, finalMessage);
        if (this.hooks.onTaskCompleted) {
          this.hooks.onTaskCompleted(sessionId, finalMessage.turnId || '');
        }
        this.hooks.syncTimeline();
        this.pageState.setBusy(false);
        this.pageState.setServiceState(GeneralChatServiceState.Ready);
        this.pageState.setStatus(RemoteI18n.t('generalChat.ready'));
      }
    } catch (err) {
      await this.handleSendFailure(sessionId, rawText, images, err);
    }
  }

  stop(cancelled: boolean, finalStatus: string = 'cancelled'): void {
    const hasStream = this.stream.hasActiveStream();
    if (!this.hooks.isVisible(this.pageState.activeSession.sessionId || '') && !hasStream) {
      return;
    }
    const hadStream = hasStream || this.pageState.activeTurnMessage.id.length > 0;
    const currentSessionId = this.stream.currentSessionId();
    const currentTurnId = this.stream.currentTurnId() || this.hooks.currentActiveTurnId();
    const requestInFlight = this.stream.isRequestInFlight();
    if (cancelled && currentSessionId.length > 0) {
      this.flushDelta(currentSessionId);
    }
    if (hasStream) {
      this.stream.markRequestCancelled();
      this.command.cancelCurrentRequest();
    }
    if (hadStream && this.hooks.onTaskStopped) {
      this.hooks.onTaskStopped(currentSessionId || this.pageState.activeSession.sessionId || '', currentTurnId);
    }
    this.stream.reset();
    if (cancelled && this.pageState.activeTurnMessage.id.length > 0) {
      const active = this.pageState.activeTurnMessage;
      const stopped: ChatMessage = {
        id: Encoding.randomId('assistant'),
        turnId: this.hooks.currentActiveTurnId(),
        role: 'assistant',
        text: active.text || (finalStatus === 'failed' ?
          RemoteI18n.t('generalChat.interrupted') : RemoteI18n.t('generalChat.stopped')),
        status: finalStatus,
        detail: finalStatus === 'failed' ? this.hooks.latestUserMessageText() : '',
        timestamp: new Date().toISOString(),
        items: this.assistantItems([], active.text || '')
      };
      this.timeline.dispatch({
        type: 'turn_finished',
        sessionId: this.pageState.activeSession.sessionId || '',
        turnId: stopped.turnId || this.hooks.currentActiveTurnId(),
        message: stopped
      });
      if (requestInFlight) {
        this.stream.setPendingFinalMessage(stopped);
      } else {
        this.persistAssistantMessage(this.pageState.activeSession.sessionId || '', stopped);
      }
      this.pageState.setStatus(finalStatus === 'failed' ?
        RemoteI18n.t('generalChat.interrupted') : RemoteI18n.t('status.stopRequested'));
      this.pageState.setServiceState(finalStatus === 'failed' ?
        GeneralChatServiceState.Failed : GeneralChatServiceState.Ready);
    }
    if (this.hooks.isVisible(this.pageState.activeSession.sessionId || '')) {
      this.hooks.syncTimeline();
      if (cancelled || hadStream) {
        this.pageState.setBusy(false);
      }
    }
  }

  reset(): void {
    this.stream.reset();
  }

  private beginStream(sessionId: string): void {
    const activeTurn = this.stream.begin(sessionId);
    const turnId = activeTurn.turnId || activeTurn.id.replace('active-', '');
    this.timeline.dispatch({
      type: 'turn_started',
      sessionId,
      turnId
    });
    if (this.hooks.onTaskStarted) {
      this.hooks.onTaskStarted(sessionId, turnId);
    }
    this.hooks.syncTimeline();
  }

  private appendDelta(sessionId: string, delta: string): void {
    const accepted = this.stream.append(
      sessionId,
      delta,
      (): boolean => this.hooks.isVisible(sessionId),
      (activeTurn: ChatMessage): void => {
        this.timeline.dispatch({ type: 'active_turn_updated', sessionId, message: activeTurn });
        this.hooks.syncTimeline();
      }
    );
    if (accepted) {
      this.pageState.setServiceState(GeneralChatServiceState.Streaming);
    }
  }

  private flushDelta(sessionId: string): void {
    const activeTurn = this.stream.flush(sessionId, this.hooks.isVisible(sessionId));
    if (!activeTurn) {
      return;
    }
    this.timeline.dispatch({ type: 'active_turn_updated', sessionId, message: activeTurn });
    this.hooks.syncTimeline();
  }

  private async finishStream(sessionId: string, result: GeneralChatSendResult): Promise<void> {
    const currentSessionId = this.stream.currentSessionId();
    const visible = this.hooks.isVisible(sessionId);
    RemoteLogger.info(`general chat finish check session=${this.shortId(sessionId)} current=${this.shortId(currentSessionId)} visible=${visible}`);
    if (currentSessionId !== sessionId && !visible) {
      return;
    }
    const finalMessage = this.messageForActiveTurn(result.assistantMessage);
    this.timeline.dispatch({ type: 'user_message', message: result.userMessage, persisted: true });
    this.timeline.dispatch({
      type: 'turn_finished',
      sessionId,
      turnId: finalMessage.turnId || this.hooks.currentActiveTurnId(),
      message: finalMessage
    });
    await this.persistAssistantMessage(sessionId, finalMessage);
    if (this.hooks.onTaskCompleted) {
      this.hooks.onTaskCompleted(sessionId, finalMessage.turnId || '');
    }
    this.hooks.syncTimeline();
    this.resetStream();
    this.pageState.setBusy(false);
    this.pageState.setServiceState(GeneralChatServiceState.Ready);
    this.pageState.setStatus(RemoteI18n.t('generalChat.ready'));
  }

  private messageForActiveTurn(message: ChatMessage): ChatMessage {
    const turnId = this.stream.currentTurnId() || message.turnId;
    return {
      id: message.id,
      role: message.role,
      text: message.text,
      status: message.status,
      renderVersion: message.renderVersion,
      turnId,
      detail: message.detail,
      error: message.error,
      timestamp: message.timestamp,
      thinking: message.thinking,
      tools: message.tools,
      items: message.items,
      images: message.images
    };
  }

  private async handleSendFailure(
    sessionId: string,
    rawText: string,
    images: SelectedImageAttachment[],
    err: Object
  ): Promise<void> {
    if (this.hooks.onTaskStopped) {
      this.hooks.onTaskStopped(
        sessionId,
        this.stream.currentTurnId() || this.hooks.currentActiveTurnId()
      );
    }
    this.stream.setRequestInFlight(false);
    if (this.stream.takeRequestCancelled()) {
      const finalMessage = this.stream.takePendingFinalMessage();
      if (finalMessage) {
        await this.persistAssistantMessage(sessionId, finalMessage);
        this.pageState.setServiceState(finalMessage.status === 'failed' ?
          GeneralChatServiceState.Failed : GeneralChatServiceState.Ready);
      } else {
        this.pageState.setServiceState(GeneralChatServiceState.Ready);
      }
      this.resetStream();
      this.pageState.setBusy(false);
      return;
    }
    this.flushDelta(sessionId);
    const partialText = this.stream.text();
    const errorText = ConnectionErrorPolicy.errorText(err);
    this.pageState.setServiceState(GeneralChatServiceStatus.fromError(errorText));
    this.resetStream();
    this.pageState.setChatInput(rawText);
    this.pageState.setSelectedImages(images);
    await this.drafts.saveNow(sessionId, rawText);
    this.timeline.setPersistedMessages(await this.command.loadMessages(sessionId));
    if (partialText.trim().length > 0) {
      const interrupted: ChatMessage = {
        id: Encoding.randomId('assistant'),
        turnId: this.hooks.currentActiveTurnId(),
        role: 'assistant',
        text: partialText,
        status: 'failed',
        detail: rawText,
        timestamp: new Date().toISOString(),
        items: this.assistantItems([], partialText)
      };
      this.timeline.dispatch({
        type: 'turn_finished',
        sessionId,
        turnId: interrupted.turnId || this.hooks.currentActiveTurnId(),
        message: interrupted
      });
      await this.persistAssistantMessage(sessionId, interrupted);
    } else {
      this.timeline.dispatch({ type: 'error', message: errorText });
    }
    this.hooks.syncTimeline();
    this.pageState.setStatus(errorText);
    this.pageState.setBusy(false);
  }

  private resetStream(): void {
    this.stream.reset();
  }

  private async persistAssistantMessage(sessionId: string, message: ChatMessage): Promise<void> {
    if (sessionId.length > 0) {
      await this.command.recordAssistantMessage(sessionId, message);
    }
  }

  private localUserMessage(text: string, images: SelectedImageAttachment[]): ChatMessage {
    return {
      id: Encoding.randomId('optimistic-user'),
      turnId: '',
      role: 'user',
      text,
      status: 'sent',
      detail: '',
      timestamp: new Date().toISOString(),
      images,
      items: []
    };
  }

  private assistantItems(tools: RemoteToolStatusResponse[], text: string): ChatMessageItemResponse[] {
    const items: ChatMessageItemResponse[] = [];
    tools.forEach((tool: RemoteToolStatusResponse) => items.push({ type: 'tool', tool }));
    if (text.trim().length > 0) {
      items.push({ type: 'text', content: text });
    }
    return items;
  }

  private shortId(value: string): string {
    return value.length > 8 ? value.slice(0, 8) : value;
  }
}
