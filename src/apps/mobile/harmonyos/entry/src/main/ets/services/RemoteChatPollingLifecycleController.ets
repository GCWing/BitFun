import { ChatMessage } from '../model/RemoteModels';
import {
  ChatSessionController,
  ChatSessionControllerCallbacks,
  ChatSessionCursor,
  ChatSessionSnapshot
} from './ChatSessionController';
import { RemoteSessionManager } from './RemoteSessionManager';

export type RemoteChatPollingCursor = ChatSessionCursor;
export type RemoteChatPollingSnapshot = ChatSessionSnapshot;

export interface RemoteChatPollingStartSnapshot {
  sessionId: string;
  cursor: RemoteChatPollingCursor;
  activeTurn?: ChatMessage;
}

export type RemoteChatPollingLifecycleCallbacks = ChatSessionControllerCallbacks;

export class RemoteChatPollingLifecycleController {
  private readonly controller: ChatSessionController;

  constructor(sessionManager: RemoteSessionManager, callbacks: ChatSessionControllerCallbacks) {
    this.controller = new ChatSessionController(sessionManager, callbacks);
  }

  startActiveSession(snapshot: RemoteChatPollingStartSnapshot): void {
    if (snapshot.sessionId.length === 0) {
      return;
    }
    this.controller.start(snapshot.sessionId, snapshot.cursor, snapshot.activeTurn);
  }

  stop(): void {
    this.controller.stop();
  }

  nudge(): void {
    this.controller.nudge();
  }

  async pollNow(): Promise<void> {
    await this.controller.pollNow();
  }

  updateCursor(cursor: RemoteChatPollingCursor): void {
    this.controller.updateCursor(cursor);
  }

  updateKnownModelCatalogVersion(version: number): void {
    this.controller.updateKnownModelCatalogVersion(version);
  }
}
