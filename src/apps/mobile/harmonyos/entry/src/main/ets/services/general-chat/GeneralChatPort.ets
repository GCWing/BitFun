import { ChatMessage, ImageAttachment, RemoteSession, RemoteToolStatusResponse } from '../../model/RemoteModels';

export interface GeneralChatSendResult {
  userMessage: ChatMessage;
  assistantMessage: ChatMessage;
  streamChunks: string[];
  tools: RemoteToolStatusResponse[];
}

export interface GeneralChatStreamObserver {
  onUserMessage(message: ChatMessage): Promise<void>;
  onDelta(delta: string): void;
}

export class GeneralChatStreamCallbacks implements GeneralChatStreamObserver {
  private readonly userMessageCallback: (message: ChatMessage) => Promise<void>;
  private readonly deltaCallback: (delta: string) => void;

  constructor(
    userMessageCallback: (message: ChatMessage) => Promise<void>,
    deltaCallback: (delta: string) => void
  ) {
    this.userMessageCallback = userMessageCallback;
    this.deltaCallback = deltaCallback;
  }

  async onUserMessage(message: ChatMessage): Promise<void> {
    await this.userMessageCallback(message);
  }

  onDelta(delta: string): void {
    this.deltaCallback(delta);
  }
}

export interface GeneralChatPort {
  createSession(firstMessage: string): Promise<RemoteSession>;
  sendMessage(
    sessionId: string,
    text: string,
    images?: ImageAttachment[],
    history?: ChatMessage[],
    observer?: GeneralChatStreamObserver
  ): Promise<GeneralChatSendResult>;
  cancelCurrentRequest(): void;
  generateTitle(firstMessage: string, assistantMessage: string): Promise<string>;
}

export interface GeneralChatLocalStore {
  init(context: Context): Promise<void>;
  listSessions(): Promise<RemoteSession[]>;
  loadMessages(sessionId: string): Promise<ChatMessage[]>;
  upsertSession(session: RemoteSession): Promise<void>;
  replaceMessages(sessionId: string, messages: ChatMessage[]): Promise<void>;
  deleteSession(sessionId: string): Promise<void>;
  loadDraft(draftId: string): Promise<string>;
  saveDraft(draftId: string, text: string): Promise<void>;
  deleteDraft(draftId: string): Promise<void>;
}

export interface GeneralChatDraftStore {
  draft(draftId: string): Promise<string>;
  saveDraft(draftId: string, text: string): Promise<void>;
  clearDraft(draftId: string): Promise<void>;
}
