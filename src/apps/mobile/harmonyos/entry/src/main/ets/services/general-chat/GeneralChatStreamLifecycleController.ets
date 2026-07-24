import { ChatMessage } from '../../model/RemoteModels';
import { GeneralChatStreamCoordinator } from './GeneralChatStreamCoordinator';

export class GeneralChatStreamLifecycleController {
  private readonly coordinator: GeneralChatStreamCoordinator = new GeneralChatStreamCoordinator();
  private requestCancelled: boolean = false;
  private requestInFlight: boolean = false;
  private pendingFinalMessage?: ChatMessage;

  begin(sessionId: string): ChatMessage {
    this.requestCancelled = false;
    this.pendingFinalMessage = undefined;
    return this.coordinator.begin(sessionId);
  }

  append(
    sessionId: string,
    delta: string,
    isVisible: () => boolean,
    onFlush: (message: ChatMessage) => void
  ): boolean {
    return this.coordinator.append(sessionId, delta, isVisible, onFlush);
  }

  flush(sessionId: string, visible: boolean): ChatMessage | undefined {
    return this.coordinator.flush(sessionId, visible);
  }

  reset(): void {
    this.coordinator.reset();
  }

  text(): string {
    return this.coordinator.text();
  }

  currentSessionId(): string {
    return this.coordinator.currentSessionId();
  }

  currentTurnId(): string {
    return this.coordinator.currentTurnId();
  }

  hasActiveStream(): boolean {
    return this.coordinator.hasActiveStream();
  }

  setRequestInFlight(requestInFlight: boolean): void {
    this.requestInFlight = requestInFlight;
  }

  isRequestInFlight(): boolean {
    return this.requestInFlight;
  }

  markRequestCancelled(): void {
    this.requestCancelled = true;
  }

  takeRequestCancelled(): boolean {
    const cancelled = this.requestCancelled;
    this.requestCancelled = false;
    return cancelled;
  }

  setPendingFinalMessage(message: ChatMessage): void {
    this.pendingFinalMessage = message;
  }

  takePendingFinalMessage(): ChatMessage | undefined {
    const message = this.pendingFinalMessage;
    this.pendingFinalMessage = undefined;
    return message;
  }
}
