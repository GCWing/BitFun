import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { api } from './ApiClient';

export const FEEDBACK_CONTENT_MAX_CHARS = 2_000;

export type FeedbackCategory = 'runtime_error' | 'feature_request' | 'usage_question' | 'other';
export type FeedbackStatus = 'submitted' | 'in_progress' | 'waiting_user' | 'resolved';
export type FeedbackSender = 'user' | 'admin';

export interface SubmitFeedbackInput {
  category: FeedbackCategory;
  content: string;
  includeCorrelation: boolean;
}

export interface SubmitFeedbackResult {
  feedbackId: string;
  status: FeedbackStatus;
  inboxCursor: string;
}

export interface FeedbackRecordSummary {
  feedbackId: string;
  category: FeedbackCategory;
  status: FeedbackStatus;
  hasNewReply: boolean;
  createdAt: string;
  updatedAt: string;
  canOpen: boolean;
}

export interface FeedbackInboxPage {
  items: FeedbackRecordSummary[];
  nextCursor?: string;
  hasMore: boolean;
}

export interface FeedbackAccessState {
  hasHistory: boolean;
  canReuseAccess: boolean;
  cachedInbox: FeedbackInboxPage;
}

export interface ListFeedbackRecordsInput {
  cursor?: string;
  pageSize?: number;
}

export interface FeedbackMessage {
  messageId: string;
  sender: FeedbackSender;
  content: string;
  contentDeleted: boolean;
  createdAt: string;
}

export interface FeedbackConversationPage {
  messages: FeedbackMessage[];
  nextCursor?: string;
  hasMore: boolean;
  syncError?: FeedbackApiError;
}

export interface AcknowledgeFeedbackResult {
  readThrough: string;
  feedbackStatus: FeedbackStatus;
}

export interface ReplyFeedbackResult {
  message: FeedbackMessage;
  feedbackStatus: FeedbackStatus;
}

interface FeedbackCommandErrorShape {
  code?: string;
  message?: string;
  retryable?: boolean;
  requestId?: string;
  retryAfterSeconds?: number;
}

export class FeedbackApiError extends Error {
  constructor(
    public readonly code: string,
    message: string,
    public readonly retryable: boolean,
    public readonly requestId?: string,
    public readonly retryAfterSeconds?: number,
  ) {
    super(message);
    this.name = 'FeedbackApiError';
  }
}

export class FeedbackAPI {
  correlationAvailable(): boolean {
    return Boolean(flowChatStore.getState().activeSessionId);
  }

  async submitFeedback(input: SubmitFeedbackInput): Promise<SubmitFeedbackResult> {
    const submit = await this.prepareSubmission(input);
    return submit();
  }

  async getAccessState(): Promise<FeedbackAccessState> {
    return this.invoke<FeedbackAccessState>('feedback_get_access_state', {});
  }

  async listFeedbackRecords(
    input: ListFeedbackRecordsInput,
    options: { userInitiated: boolean },
  ): Promise<FeedbackInboxPage> {
    return this.invoke<FeedbackInboxPage>('list_feedback', {
      cursor: input.cursor,
      pageSize: input.pageSize ?? 20,
      userInitiated: options.userInitiated,
    });
  }

  async openConversation(input: {
    feedbackId: string;
    cursor?: string;
    pageSize?: number;
  }): Promise<FeedbackConversationPage> {
    const page = await this.invoke<FeedbackConversationPage>('open_feedback_conversation', {
      feedbackId: input.feedbackId,
      cursor: input.cursor,
      pageSize: input.pageSize ?? 50,
      userInitiated: true,
    });
    return {
      ...page,
      syncError: page.syncError ? normalizeFeedbackError(page.syncError) : undefined,
    };
  }

  async acknowledgeFeedback(
    feedbackId: string,
    lastVisibleAt: string,
  ): Promise<AcknowledgeFeedbackResult> {
    return this.invoke<AcknowledgeFeedbackResult>('acknowledge_feedback', {
      feedbackId,
      lastVisibleAt,
      foregroundVisible: typeof document !== 'undefined'
        && document.visibilityState === 'visible',
    });
  }

  async replyFeedback(feedbackId: string, content: string): Promise<ReplyFeedbackResult> {
    return this.invoke<ReplyFeedbackResult>('reply_feedback', {
      feedbackId,
      content,
    });
  }

  async prepareSubmission(
    input: SubmitFeedbackInput,
  ): Promise<() => Promise<SubmitFeedbackResult>> {
    const sessionIdHash = input.includeCorrelation
      ? await this.activeSessionIdHash()
      : undefined;
    const request = {
      category: input.category,
      content: input.content,
      sessionIdHash,
    };
    return () => this.invoke<SubmitFeedbackResult>('submit_feedback', request);
  }

  private async invoke<T>(command: string, request: Record<string, unknown>): Promise<T> {
    try {
      return await api.invoke<T>(command, { request }, { retries: 0 });
    } catch (error) {
      throw normalizeFeedbackError(error);
    }
  }

  private async activeSessionIdHash(): Promise<string | undefined> {
    const sessionId = flowChatStore.getState().activeSessionId;
    if (!sessionId || !globalThis.crypto?.subtle) return undefined;
    const bytes = new TextEncoder().encode(sessionId);
    const digest = await globalThis.crypto.subtle.digest('SHA-256', bytes);
    return Array.from(
      new Uint8Array(digest),
      value => value.toString(16).padStart(2, '0'),
    ).join('');
  }
}

export function truncateFeedbackContent(value: string): string {
  const characters = Array.from(value);
  return characters.length <= FEEDBACK_CONTENT_MAX_CHARS
    ? value
    : characters.slice(0, FEEDBACK_CONTENT_MAX_CHARS).join('');
}

export function feedbackContentLength(value: string): number {
  return Array.from(value).length;
}

export function normalizeFeedbackError(error: unknown): FeedbackApiError {
  if (error instanceof FeedbackApiError) return error;
  const value = error as FeedbackCommandErrorShape | null;
  if (value && typeof value === 'object' && typeof value.code === 'string') {
    return fromShape(value);
  }
  const message = error instanceof Error ? error.message : String(error);
  const structured = parseStructuredError(message);
  if (structured?.code) return fromShape(structured);
  return new FeedbackApiError('SERVICE_UNAVAILABLE', 'Feedback service is unavailable', true);
}

function fromShape(value: FeedbackCommandErrorShape): FeedbackApiError {
  return new FeedbackApiError(
    value.code ?? 'UNKNOWN_ERROR',
    value.message ?? 'Feedback request could not be completed',
    value.retryable ?? false,
    value.requestId,
    value.retryAfterSeconds,
  );
}

function parseStructuredError(message: string): FeedbackCommandErrorShape | null {
  const start = message.indexOf('{');
  if (start < 0) return null;
  try {
    return JSON.parse(message.slice(start)) as FeedbackCommandErrorShape;
  } catch {
    return null;
  }
}

export const feedbackAPI = new FeedbackAPI();
