import { ChatMessage, ChatMessageItemResponse } from '../../model/RemoteModels';
import { Encoding } from '../Encoding';

export class GeneralChatStreamCoordinator {
  private sessionId: string = '';
  private turnId: string = '';
  private accumulatedText: string = '';
  private pendingDelta: string = '';
  private deltaTimerId: number = 0;

  begin(sessionId: string, turnId: string = ''): ChatMessage {
    this.reset();
    this.sessionId = sessionId;
    this.turnId = turnId.length > 0 ? turnId : Encoding.randomId('turn');
    return this.activeMessage();
  }

  append(
    sessionId: string,
    delta: string,
    isVisible: () => boolean,
    onFlush: (message: ChatMessage) => void
  ): boolean {
    if (delta.length === 0 || this.sessionId !== sessionId || !isVisible()) {
      return false;
    }
    this.pendingDelta += delta;
    if (this.deltaTimerId === 0) {
      this.deltaTimerId = setTimeout(() => {
        this.deltaTimerId = 0;
        const message = this.flush(sessionId, isVisible());
        if (message) {
          onFlush(message);
        }
      }, 32);
    }
    return true;
  }

  flush(sessionId: string, visible: boolean): ChatMessage | undefined {
    if (this.pendingDelta.length === 0 || this.sessionId !== sessionId || !visible) {
      return undefined;
    }
    this.accumulatedText = (this.accumulatedText + this.pendingDelta).replace(/\n{3,}/g, '\n\n');
    this.pendingDelta = '';
    return this.activeMessage();
  }

  reset(): void {
    if (this.deltaTimerId !== 0) {
      clearTimeout(this.deltaTimerId);
      this.deltaTimerId = 0;
    }
    this.sessionId = '';
    this.turnId = '';
    this.accumulatedText = '';
    this.pendingDelta = '';
  }

  currentSessionId(): string {
    return this.sessionId;
  }

  currentTurnId(): string {
    return this.turnId;
  }

  text(): string {
    return this.accumulatedText;
  }

  hasActiveStream(): boolean {
    return this.sessionId.length > 0 || this.turnId.length > 0 || this.deltaTimerId !== 0;
  }

  private activeMessage(): ChatMessage {
    const items: ChatMessageItemResponse[] = [];
    if (this.accumulatedText.trim().length > 0) {
      items.push({
        type: 'text',
        content: this.accumulatedText
      });
    }
    return {
      id: `active-${this.turnId}`,
      turnId: this.turnId,
      role: 'assistant',
      text: this.accumulatedText,
      status: 'active',
      detail: '',
      items,
      renderVersion: this.accumulatedText.length
    };
  }
}
