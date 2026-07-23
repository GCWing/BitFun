import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export interface EditorAiStreamRequest {
  requestId: string;
  prompt: string;
  modelId?: string;
}

export interface EditorAiStreamResponse {
  ok: boolean;
}

export interface EditorAiCancelRequest {
  requestId: string;
}

export interface EditorAiTextChunkEvent {
  requestId: string;
  text: string;
}

export interface EditorAiCompletedEvent {
  requestId: string;
  fullText: string;
  finishReason?: string | null;
}

export interface EditorAiErrorEvent {
  requestId: string;
  error: string;
}

export class EditorAiAPI {
  async stream(request: EditorAiStreamRequest): Promise<EditorAiStreamResponse> {
    try {
      return await api.invoke<EditorAiStreamResponse>('editor_ai_stream', { request });
    } catch (error) {
      throw createTauriCommandError('editor_ai_stream', error, request);
    }
  }

  async cancel(request: EditorAiCancelRequest): Promise<void> {
    try {
      await api.invoke<void>('editor_ai_cancel', { request });
    } catch (error) {
      throw createTauriCommandError('editor_ai_cancel', error, request);
    }
  }

  onTextChunk(callback: (event: EditorAiTextChunkEvent) => void): () => void {
    return api.listen<EditorAiTextChunkEvent>('editor-ai://text-chunk', callback);
  }

  onCompleted(callback: (event: EditorAiCompletedEvent) => void): () => void {
    return api.listen<EditorAiCompletedEvent>('editor-ai://completed', callback);
  }

  onError(callback: (event: EditorAiErrorEvent) => void): () => void {
    return api.listen<EditorAiErrorEvent>('editor-ai://error', callback);
  }
}

export const editorAiAPI = new EditorAiAPI();
