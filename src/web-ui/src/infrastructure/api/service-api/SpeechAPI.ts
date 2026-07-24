import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export const LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID = 'sensevoice-small-int8';
export const LOCAL_QWEN3_ASR_0_6B_INT8_MODEL_ID = 'qwen3-asr-0.6b-int8';
export const DEFAULT_SPEECH_SAMPLE_RATE = 16000;
export const DEFAULT_MAX_RECORDING_SECONDS = 60;
export const SPEECH_MODEL_PROGRESS_EVENT = 'speech://model-download-progress';
export const SPEECH_MODEL_STATUS_CHANGED_EVENT = 'speech://model-status-changed';

export type SpeechModelInstallState =
  | 'not_installed' | 'downloading' | 'installed' | 'verifying'
  | 'corrupt' | 'deleting' | 'error';

export interface SpeechModelProgress {
  modelId: string;
  downloadedBytes: number;
  totalBytes: number;
  percent: number;
}

export interface SpeechModelStatus {
  modelId: string;
  displayName: string;
  provider: string;
  version: string;
  description: string;
  languages: string[];
  state: SpeechModelInstallState;
  installedPath?: string | null;
  installedBytes: number;
  expectedBytes: number;
  progress?: SpeechModelProgress | null;
  error?: string | null;
}

export interface SpeechListModelsResponse { models: SpeechModelStatus[]; }
export interface SpeechModelProgressEvent { status: SpeechModelStatus; }
export interface SpeechStartInputSessionRequest {
  modelId?: string | null;
  language?: string | null;
  sampleRate?: number | null;
  maxRecordingSeconds?: number | null;
}
export interface SpeechInputSession {
  sessionId: string;
  modelId: string;
  language: string;
  sampleRate: number;
  maxRecordingSeconds: number;
}
export interface SpeechAppendAudioChunkResponse {
  receivedBytes: number;
  receivedSeconds: number;
  limitReached: boolean;
}
export interface SpeechTranscriptionResult {
  text: string;
  language: string;
  durationMs: number;
  audioDurationSeconds: number;
}

export class SpeechAPI {
  async listModels() {
    try { return await api.invoke<SpeechListModelsResponse>('speech_list_models', {}); }
    catch (error) { throw createTauriCommandError('speech_list_models', error); }
  }

  async downloadModel(modelId: string) {
    try { return await api.invoke<SpeechModelStatus>('speech_download_model', { request: { modelId } }, { timeout: 600000 }); }
    catch (error) { throw createTauriCommandError('speech_download_model', error, { modelId }); }
  }

  async cancelModelDownload(modelId: string) {
    try { return await api.invoke<SpeechModelStatus>('speech_cancel_model_download', { request: { modelId } }); }
    catch (error) { throw createTauriCommandError('speech_cancel_model_download', error, { modelId }); }
  }

  async deleteModel(modelId: string) {
    try { return await api.invoke<SpeechModelStatus>('speech_delete_model', { request: { modelId } }); }
    catch (error) { throw createTauriCommandError('speech_delete_model', error, { modelId }); }
  }

  async verifyModel(modelId: string) {
    try { return await api.invoke<SpeechModelStatus>('speech_verify_model', { request: { modelId } }); }
    catch (error) { throw createTauriCommandError('speech_verify_model', error, { modelId }); }
  }

  async startInputSession(request: SpeechStartInputSessionRequest = {}) {
    try { return await api.invoke<SpeechInputSession>('speech_start_input_session', { request }); }
    catch (error) { throw createTauriCommandError('speech_start_input_session', error, request); }
  }

  async appendAudioChunk(sessionId: string, pcm16Base64: string) {
    try {
      return await api.invoke<SpeechAppendAudioChunkResponse>('speech_append_audio_chunk', { request: { sessionId, pcm16Base64 } });
    } catch (error) { throw createTauriCommandError('speech_append_audio_chunk', error, { sessionId }); }
  }

  async finishInputSession(sessionId: string) {
    try {
      return await api.invoke<SpeechTranscriptionResult>('speech_finish_input_session', { request: { sessionId } }, { timeout: 120000 });
    } catch (error) { throw createTauriCommandError('speech_finish_input_session', error, { sessionId }); }
  }

  async cancelInputSession(sessionId: string): Promise<void> {
    try { await api.invoke('speech_cancel_input_session', { request: { sessionId } }); }
    catch (error) { throw createTauriCommandError('speech_cancel_input_session', error, { sessionId }); }
  }

  onModelProgress(callback: (event: SpeechModelProgressEvent) => void): () => void {
    return api.listen(SPEECH_MODEL_PROGRESS_EVENT, callback);
  }

  onModelStatusChanged(callback: (status: SpeechModelStatus) => void): () => void {
    return api.listen(SPEECH_MODEL_STATUS_CHANGED_EVENT, callback);
  }
}

export const speechAPI = new SpeechAPI();
