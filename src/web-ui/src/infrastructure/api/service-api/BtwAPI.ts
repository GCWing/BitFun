import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export interface BtwAskRequest {
  sessionId: string;
  question: string;
  /** Optional model id override. Supports "fast"/"primary" aliases. */
  modelId?: string;
  /** Limit how many context messages are included (from the end). */
  maxContextMessages?: number;
}

export interface BtwAskResponse {
  answer: string;
}

export interface BtwAskStreamRequest extends BtwAskRequest {
  requestId: string;
}

export interface BtwAskStreamResponse {
  ok: boolean;
}

export interface BtwCancelRequest {
  requestId: string;
}

export interface BtwTextChunkEvent {
  requestId: string;
  sessionId: string;
  text: string;
}

export interface BtwCompletedEvent {
  requestId: string;
  sessionId: string;
  fullText: string;
  finishReason?: string | null;
}

export interface BtwErrorEvent {
  requestId: string;
  sessionId: string;
  error: string;
}

export class BtwAPI {
  async ask(request: BtwAskRequest): Promise<BtwAskResponse> {
    try {
      return await api.invoke<BtwAskResponse>('btw_ask', { request });
    } catch (error) {
      throw createTauriCommandError('btw_ask', error, request);
    }
  }

  async askStream(request: BtwAskStreamRequest): Promise<BtwAskStreamResponse> {
    try {
      return await api.invoke<BtwAskStreamResponse>('btw_ask_stream', { request });
    } catch (error) {
      throw createTauriCommandError('btw_ask_stream', error, request);
    }
  }

  async cancel(request: BtwCancelRequest): Promise<void> {
    try {
      await api.invoke<void>('btw_cancel', { request });
    } catch (error) {
      throw createTauriCommandError('btw_cancel', error, request);
    }
  }
}

export const btwAPI = new BtwAPI();
