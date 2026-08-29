import {
  ChatMessage,
  PollSessionResult,
  RemoteModelCatalog
} from '../model/RemoteModels';
import { RemoteLogger } from './RemoteLogger';
import { RemoteSessionManager } from './RemoteSessionManager';

export interface ChatSessionCursor {
  pollVersion: number;
  knownMessageCount: number;
  knownModelCatalogVersion: number;
}

export interface ChatSessionSnapshot {
  sessionId: string;
  cursor: ChatSessionCursor;
  changed: boolean;
  title: string;
  sessionState: string;
  newMessages: ChatMessage[];
  messageSnapshot?: ChatMessage[];
  activeTurn?: ChatMessage;
  modelCatalog?: RemoteModelCatalog;
  shouldSyncAfterTurnEnded: boolean;
  // The desktop now holds fewer messages than this session had counted, so the
  // transcript it was handing out tails of is no longer the one on screen.
  // Nothing below can resume from an offset into a list that shrank.
  historyRewritten: boolean;
  /** Exact completed turn whose terminal assistant is now durable. */
  completedTurnId: string;
}

export interface ChatSessionControllerCallbacks {
  onSnapshot: (snapshot: ChatSessionSnapshot) => void;
  onError: (error: Object) => void;
  canPoll: (sessionId: string) => boolean;
}

export interface ChatSessionControllerOptions {
  /** Stabilization window before the first idle poll after selecting a session. */
  initialPollDelayMs: number;
}

// Mobile streaming is projected from poll_session.active_turn snapshots.
// The relay protocol remains request/response HTTP, not token-level SSE.
export class ChatSessionController {
  private readonly sessionManager: RemoteSessionManager;
  private readonly callbacks: ChatSessionControllerCallbacks;
  private readonly initialPollDelayMs: number;
  private sessionId: string = '';
  private cursor: ChatSessionCursor = {
    pollVersion: 0,
    knownMessageCount: 0,
    knownModelCatalogVersion: 0
  };
  private activeTurn?: ChatMessage;
  private timerId: number = 0;
  private polling: boolean = false;
  private stopped: boolean = true;
  // Invalidates in-flight requests when the session or lifecycle changes.
  private generation: number = 0;
  private hasActiveRunningTurn: boolean = false;
  private awaitingPersistedTurnId: string = '';
  private turnJustEndedAt: number = 0;
  private readonly activeIntervalMs: number = 350;
  private readonly settleIntervalMs: number = 500;
  private readonly idleIntervalMs: number = 10000;
  private readonly turnEndedGracePeriodMs: number = 5000;

  constructor(
    sessionManager: RemoteSessionManager,
    callbacks: ChatSessionControllerCallbacks,
    options?: ChatSessionControllerOptions
  ) {
    this.sessionManager = sessionManager;
    this.callbacks = callbacks;
    this.initialPollDelayMs = options ? Math.max(0, options.initialPollDelayMs) : 0;
  }

  start(sessionId: string, cursor: ChatSessionCursor, activeTurn?: ChatMessage): void {
    this.stop(false);
    this.sessionId = sessionId;
    this.cursor = {
      pollVersion: cursor.pollVersion,
      knownMessageCount: cursor.knownMessageCount,
      knownModelCatalogVersion: cursor.knownModelCatalogVersion
    };
    this.activeTurn = activeTurn && activeTurn.id.length > 0 ? activeTurn : undefined;
    this.hasActiveRunningTurn = this.isRunningTurn(this.activeTurn);
    this.awaitingPersistedTurnId = this.isRunningTurn(this.activeTurn) || this.isCompletedTurn(this.activeTurn) ?
      this.turnId(this.activeTurn) : '';
    this.turnJustEndedAt = 0;
    this.stopped = false;
    RemoteLogger.info(`poller start session=${ChatSessionController.shortId(sessionId)} active=${this.activeTurn ? this.activeTurn.id : ''}`);
    // A subsequent selection cancels this timer in stop(). This small window
    // keeps rapid cached-session taps from each putting an obsolete poll on the
    // desktop's serial RPC queue. Explicit nudge() remains immediate.
    this.scheduleNext(this.initialPollDelayMs);
  }

  stop(clearActiveTurn: boolean = true): void {
    this.generation += 1;
    if (this.timerId !== 0) {
      clearTimeout(this.timerId);
      this.timerId = 0;
    }
    this.polling = false;
    this.stopped = true;
    this.hasActiveRunningTurn = false;
    this.awaitingPersistedTurnId = '';
    this.turnJustEndedAt = 0;
    if (clearActiveTurn) {
      this.activeTurn = undefined;
    }
  }

  nudge(): void {
    if (this.sessionId.length === 0) {
      return;
    }
    this.hasActiveRunningTurn = true;
    this.turnJustEndedAt = 0;
    RemoteLogger.info(`poller nudge session=${ChatSessionController.shortId(this.sessionId)}`);
    this.scheduleNext(0);
  }

  updateCursor(cursor: ChatSessionCursor): void {
    this.cursor = {
      pollVersion: cursor.pollVersion,
      knownMessageCount: cursor.knownMessageCount,
      knownModelCatalogVersion: cursor.knownModelCatalogVersion
    };
  }

  updateKnownModelCatalogVersion(version: number): void {
    this.cursor = {
      pollVersion: this.cursor.pollVersion,
      knownMessageCount: this.cursor.knownMessageCount,
      knownModelCatalogVersion: version
    };
  }

  setKnownMessageCount(count: number): void {
    this.cursor = {
      pollVersion: this.cursor.pollVersion,
      knownMessageCount: count,
      knownModelCatalogVersion: this.cursor.knownModelCatalogVersion
    };
  }

  clearActiveTurn(): void {
    this.activeTurn = undefined;
    this.hasActiveRunningTurn = false;
    this.turnJustEndedAt = 0;
  }

  async pollNow(): Promise<void> {
    if (this.stopped || this.polling || this.sessionId.length === 0) {
      return;
    }
    if (!this.callbacks.canPoll(this.sessionId)) {
      return;
    }

    const requestGeneration = this.generation;
    const requestSessionId = this.sessionId;
    const requestCursor: ChatSessionCursor = {
      pollVersion: this.cursor.pollVersion,
      knownMessageCount: this.cursor.knownMessageCount,
      knownModelCatalogVersion: this.cursor.knownModelCatalogVersion
    };
    this.polling = true;
    try {
      RemoteLogger.info(`poller tick session=${ChatSessionController.shortId(requestSessionId)} version=${requestCursor.pollVersion} known=${requestCursor.knownMessageCount}`);
      const result = await this.sessionManager.pollSession(
        requestSessionId,
        requestCursor.pollVersion,
        requestCursor.knownMessageCount,
        requestCursor.knownModelCatalogVersion
      );
      if (!this.isCurrentRequest(requestGeneration, requestSessionId)) {
        return;
      }
      this.applyPollResult(result);
    } catch (err) {
      if (this.isCurrentRequest(requestGeneration, requestSessionId)) {
        this.callbacks.onError(err);
      }
    } finally {
      if (this.isCurrentRequest(requestGeneration, requestSessionId)) {
        this.polling = false;
        this.scheduleNext();
      }
    }
  }

  private isCurrentRequest(requestGeneration: number, requestSessionId: string): boolean {
    return requestGeneration === this.generation &&
      !this.stopped &&
      requestSessionId === this.sessionId;
  }

  private applyPollResult(result: PollSessionResult): void {
    const hadRunningTurn = this.hasActiveRunningTurn;
    const incomingMessages = result.newMessages || [];
    const persistedProjection = result.messageSnapshot || incomingMessages;
    const hasAssistantMessage = persistedProjection.some((message: ChatMessage) => {
      return message.role === 'assistant';
    });
    const historyRewritten = result.totalMessageCount > 0 &&
      result.totalMessageCount < this.cursor.knownMessageCount;

    if (result.changed) {
      this.cursor = {
        pollVersion: result.version,
        knownMessageCount: result.totalMessageCount > 0
          ? result.totalMessageCount
          : this.cursor.knownMessageCount,
        knownModelCatalogVersion: result.modelCatalog
          ? (result.modelCatalog.version || 0)
          : this.cursor.knownModelCatalogVersion
      };
    }

    if (result.activeTurn && result.activeTurn.id.length > 0) {
      this.activeTurn = result.activeTurn;
      if (this.isRunningTurn(result.activeTurn) || this.isCompletedTurn(result.activeTurn)) {
        this.awaitingPersistedTurnId = this.turnId(result.activeTurn);
      }
    } else if (result.changed && (hasAssistantMessage || this.shouldClearMissingActiveTurn(result))) {
      this.activeTurn = undefined;
    }

    const completedTurnId = this.persistedTerminalTurnId(persistedProjection);
    if (completedTurnId.length > 0) {
      this.awaitingPersistedTurnId = '';
    }

    const isRunningNow = this.isRunningTurn(this.activeTurn);
    const turnEndedNow = hadRunningTurn && !isRunningNow;
    if (turnEndedNow) {
      this.turnJustEndedAt = Date.now();
    }
    this.hasActiveRunningTurn = isRunningNow;

    const isSettlingEndedTurn = this.turnJustEndedAt > 0 &&
      Date.now() - this.turnJustEndedAt < this.turnEndedGracePeriodMs;
    const activeTurnChanged = result.activeTurn !== undefined;
    this.emitSnapshot(
      result.changed || activeTurnChanged,
      result,
      incomingMessages,
      result.messageSnapshot,
      turnEndedNow || (isSettlingEndedTurn && !isRunningNow),
      historyRewritten,
      completedTurnId
    );
  }

  private shouldClearMissingActiveTurn(result: PollSessionResult): boolean {
    if (!this.activeTurn || this.activeTurn.id.length === 0) {
      return false;
    }
    const state = (result.sessionState || '').toLowerCase();
    const previousStatus = (this.activeTurn.status || '').toLowerCase();
    return state === 'idle' && previousStatus !== 'completed';
  }

  private emitSnapshot(
    changed: boolean,
    result: PollSessionResult,
    newMessages: ChatMessage[],
    messageSnapshot: ChatMessage[] | undefined,
    shouldSyncAfterTurnEnded: boolean,
    historyRewritten: boolean,
    completedTurnId: string
  ): void {
    this.callbacks.onSnapshot({
      sessionId: this.sessionId,
      cursor: {
        pollVersion: this.cursor.pollVersion,
        knownMessageCount: this.cursor.knownMessageCount,
        knownModelCatalogVersion: this.cursor.knownModelCatalogVersion
      },
      changed,
      title: result.title || '',
      sessionState: result.sessionState || '',
      newMessages,
      messageSnapshot,
      activeTurn: this.activeTurn,
      modelCatalog: result.modelCatalog,
      shouldSyncAfterTurnEnded,
      historyRewritten,
      completedTurnId
    });
  }

  private scheduleNext(delayMs?: number): void {
    if (this.stopped || this.sessionId.length === 0) {
      return;
    }
    if (this.timerId !== 0) {
      clearTimeout(this.timerId);
      this.timerId = 0;
    }
    const nextDelay = delayMs !== undefined ? delayMs : this.pollIntervalMs();
    const timerGeneration = this.generation;
    const timerSessionId = this.sessionId;
    RemoteLogger.info(`poller schedule session=${ChatSessionController.shortId(this.sessionId)} delay=${nextDelay}`);
    this.timerId = setTimeout(() => {
      if (timerGeneration !== this.generation || timerSessionId !== this.sessionId || this.stopped) {
        return;
      }
      this.timerId = 0;
      this.pollNow();
    }, nextDelay);
  }

  private pollIntervalMs(): number {
    const now = Date.now();
    if (this.turnJustEndedAt > 0 && now - this.turnJustEndedAt < this.turnEndedGracePeriodMs) {
      return this.settleIntervalMs;
    }
    this.turnJustEndedAt = 0;
    return this.hasActiveRunningTurn ? this.activeIntervalMs : this.idleIntervalMs;
  }

  private isRunningTurn(turn?: ChatMessage): boolean {
    return !!turn && turn.id.length > 0 && (turn.status || '').toLowerCase() === 'active';
  }

  private isCompletedTurn(turn?: ChatMessage): boolean {
    if (!turn || turn.id.length === 0) {
      return false;
    }
    const status = (turn.status || '').toLowerCase();
    return status === 'completed' || status === 'done' || status === 'success';
  }

  private persistedTerminalTurnId(messages: ChatMessage[]): string {
    const expected = this.awaitingPersistedTurnId;
    if (expected.length === 0) {
      return '';
    }
    const matched = messages.some((message: ChatMessage) => {
      if (message.role !== 'assistant' || !this.isTerminalMessage(message)) {
        return false;
      }
      const messageTurnId = (message.turnId || '').trim();
      return messageTurnId === expected || message.id === `${expected}_assistant`;
    });
    return matched ? expected : '';
  }

  private isTerminalMessage(message: ChatMessage): boolean {
    const status = (message.status || '').toLowerCase();
    return status === 'completed' || status === 'done' || status === 'success';
  }

  private turnId(turn?: ChatMessage): string {
    if (!turn) {
      return '';
    }
    const explicit = (turn.turnId || '').trim();
    if (explicit.length > 0) {
      return explicit;
    }
    return turn.id.indexOf('active-') === 0 ? turn.id.slice('active-'.length) : '';
  }

  private static shortId(value: string): string {
    if (value.length <= 8) {
      return value;
    }
    return value.slice(0, 4) + '...' + value.slice(value.length - 4);
  }
}
