import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export interface BtwAskStreamRequest {
  requestId: string;
  sessionId: string;
  question: string;
  modelId?: string;
  childSessionId: string;
  childSessionName?: string;
}

export interface BtwAskStreamResponse {
  ok: boolean;
}

export interface BtwCancelRequest {
  requestId: string;
}

export class BtwAPI {
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
