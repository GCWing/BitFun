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
  activeTurn?: ChatMessage;
  modelCatalog?: RemoteModelCatalog;
  shouldSyncAfterTurnEnded: boolean;
}

export interface ChatSessionControllerCallbacks {
  onSnapshot: (snapshot: ChatSessionSnapshot) => void;
  onError: (error: Object) => void;
  canPoll: (sessionId: string) => boolean;
}

// Mobile streaming is projected from poll_session.active_turn snapshots.
// The relay protocol remains request/response HTTP, not token-level SSE.
export class ChatSessionController {
  private readonly sessionManager: RemoteSessionManager;
  private readonly callbacks: ChatSessionControllerCallbacks;
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
  private hasActiveRunningTurn: boolean = false;
  private turnJustEndedAt: number = 0;
  private readonly activeIntervalMs: number = 350;
  private readonly settleIntervalMs: number = 500;
  private readonly idleIntervalMs: number = 10000;
  private readonly turnEndedGracePeriodMs: number = 5000;

  constructor(sessionManager: RemoteSessionManager, callbacks: ChatSessionControllerCallbacks) {
    this.sessionManager = sessionManager;
    this.callbacks = callbacks;
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
    this.turnJustEndedAt = 0;
    this.stopped = false;
    RemoteLogger.info(`poller start session=${ChatSessionController.shortId(sessionId)} active=${this.activeTurn ? this.activeTurn.id : ''}`);
    this.scheduleNext(0);
  }

  stop(clearActiveTurn: boolean = true): void {
    if (this.timerId !== 0) {
      clearTimeout(this.timerId);
      this.timerId = 0;
    }
    this.polling = false;
    this.stopped = true;
    this.hasActiveRunningTurn = false;
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

    this.polling = true;
    try {
      RemoteLogger.info(`poller tick session=${ChatSessionController.shortId(this.sessionId)} version=${this.cursor.pollVersion} known=${this.cursor.knownMessageCount}`);
      const result = await this.sessionManager.pollSession(
        this.sessionId,
        this.cursor.pollVersion,
        this.cursor.knownMessageCount,
        this.cursor.knownModelCatalogVersion
      );
      this.applyPollResult(result);
    } catch (err) {
      this.callbacks.onError(err);
    } finally {
      this.polling = false;
      this.scheduleNext();
    }
  }

  private applyPollResult(result: PollSessionResult): void {
    if (!result.changed) {
      this.emitSnapshot(false, result, [], false);
      return;
    }

    const hadRunningTurn = this.hasActiveRunningTurn;
    const incomingMessages = result.newMessages || [];
    const hasAssistantMessage = incomingMessages.some((message: ChatMessage) => {
      return message.role === 'assistant';
    });

    this.cursor = {
      pollVersion: result.version,
      knownMessageCount: result.totalMessageCount > 0
        ? result.totalMessageCount
        : this.cursor.knownMessageCount,
      knownModelCatalogVersion: result.modelCatalog
        ? (result.modelCatalog.version || 0)
        : this.cursor.knownModelCatalogVersion
    };

    if (result.activeTurn && result.activeTurn.id.length > 0) {
      this.activeTurn = result.activeTurn;
    } else if (hasAssistantMessage || this.shouldClearMissingActiveTurn(result)) {
      this.activeTurn = undefined;
    }

    const isRunningNow = this.isRunningTurn(this.activeTurn);
    if (hadRunningTurn && !isRunningNow) {
      this.turnJustEndedAt = Date.now();
    }
    if (hasAssistantMessage) {
      this.turnJustEndedAt = 0;
    }
    this.hasActiveRunningTurn = isRunningNow;

    this.emitSnapshot(true, result, incomingMessages, hadRunningTurn && !isRunningNow);
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
    shouldSyncAfterTurnEnded: boolean
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
      activeTurn: this.activeTurn,
      modelCatalog: result.modelCatalog,
      shouldSyncAfterTurnEnded
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
    RemoteLogger.info(`poller schedule session=${ChatSessionController.shortId(this.sessionId)} delay=${nextDelay}`);
    this.timerId = setTimeout(() => {
      this.timerId = 0;
      this.pollNow();
    }, nextDelay);
  }

  private pollIntervalMs(): number {
    const now = Date.now();
    if (this.turnJustEndedAt > 0 && now - this.turnJustEndedAt < this.turnEndedGracePeriodMs) {
      return this.settleIntervalMs;
    }
    return this.hasActiveRunningTurn ? this.activeIntervalMs : this.idleIntervalMs;
  }

  private isRunningTurn(turn?: ChatMessage): boolean {
    return !!turn && turn.id.length > 0 && (turn.status || '').toLowerCase() === 'active';
  }

  private static shortId(value: string): string {
    if (value.length <= 8) {
      return value;
    }
    return value.slice(0, 4) + '...' + value.slice(value.length - 4);
  }
}
