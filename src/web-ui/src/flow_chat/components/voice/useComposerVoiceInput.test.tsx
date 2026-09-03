/**
 * @vitest-environment jsdom
 */

import React, { useEffect } from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  useComposerVoiceInput,
  type ComposerVoiceInputController,
} from './useComposerVoiceInput';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  finishText: 'Transcribed request',
  voiceInputSettings: {
    enabled: true,
    provider: 'local',
    model_id: 'sensevoice-test-model',
    default_language: 'auto',
    max_recording_seconds: 60,
    microphone_device_id: '',
  },
  recorderStop: vi.fn(async () => undefined),
  listModels: vi.fn(),
  finishInputSession: vi.fn(),
  cancelInputSession: vi.fn(async () => undefined),
  downloadModel: vi.fn(),
  cancelModelDownload: vi.fn(async () => undefined),
  modelStatusListener: undefined as ((status: Record<string, unknown>) => void) | undefined,
  notificationInfo: vi.fn(),
  notificationError: vi.fn(),
}));

vi.mock('@/infrastructure/api', () => ({
  DEFAULT_SPEECH_SAMPLE_RATE: 16000,
  LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID: 'sensevoice-test-model',
  speechAPI: {
    listModels: mocks.listModels,
    onModelStatusChanged: vi.fn((listener: (status: Record<string, unknown>) => void) => {
      mocks.modelStatusListener = listener;
      return () => undefined;
    }),
    onModelProgress: vi.fn(() => () => undefined),
    downloadModel: mocks.downloadModel,
    cancelModelDownload: mocks.cancelModelDownload,
    startInputSession: vi.fn(async () => ({ sessionId: 'voice-session-1' })),
    appendAudioChunk: vi.fn(async () => undefined),
    finishInputSession: mocks.finishInputSession,
    cancelInputSession: mocks.cancelInputSession,
  },
}));

vi.mock('@/infrastructure/config/hooks', () => ({
  useAIExperienceSettings: () => ({
    settings: {
      voice_input: mocks.voiceInputSettings,
    },
    isLoading: false,
    error: null,
  }),
}));

vi.mock('@/infrastructure/runtime', () => ({
  isTauriRuntime: () => true,
}));

vi.mock('@/app/stores/sceneStore', () => ({
  useSceneStore: {
    getState: () => ({ openScene: vi.fn() }),
  },
}));

vi.mock('@/app/scenes/settings/settingsStore', () => ({
  useSettingsStore: {
    getState: () => ({ openDestination: vi.fn() }),
  },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    info: mocks.notificationInfo,
    warning: vi.fn(),
    error: mocks.notificationError,
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('@/infrastructure/speech/voiceInputAudio', () => ({
  createVoiceInputRecorder: vi.fn(async () => ({
    stop: mocks.recorderStop,
  })),
}));

interface ProbeProps {
  focusInputSoon: () => void;
  insertText: (text: string) => string | null;
  submitText: (text: string) => Promise<void>;
  onController: (controller: ComposerVoiceInputController) => void;
}

function Probe({ onController, ...options }: ProbeProps) {
  const controller = useComposerVoiceInput(options);
  useEffect(() => onController(controller), [controller, onController]);
  return null;
}

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function speechModel(
  modelId: string,
  state: 'installed' | 'not_installed',
  expectedBytes = 1,
) {
  return {
    modelId,
    displayName: modelId === 'qwen-test-model' ? 'Qwen test' : 'SenseVoice test',
    provider: 'test',
    version: 'test',
    description: 'Test speech model',
    languages: ['auto', 'en'],
    state,
    installedBytes: state === 'installed' ? expectedBytes : 0,
    expectedBytes,
  };
}

describe('useComposerVoiceInput completion modes', () => {
  let host: HTMLDivElement;
  let root: Root;
  let controller: ComposerVoiceInputController | undefined;
  let focusInputSoon: ReturnType<typeof vi.fn>;
  let insertText: ReturnType<typeof vi.fn>;
  let submitText: ReturnType<typeof vi.fn>;

  beforeEach(async () => {
    mocks.voiceInputSettings = {
      enabled: true,
      provider: 'local',
      model_id: 'sensevoice-test-model',
      default_language: 'auto',
      max_recording_seconds: 60,
      microphone_device_id: '',
    };
    mocks.listModels.mockReset();
    mocks.listModels.mockImplementation(async () => ({
      models: [speechModel('sensevoice-test-model', 'installed')],
    }));
    mocks.finishText = 'Transcribed request';
    mocks.finishInputSession.mockImplementation(async () => ({
      text: mocks.finishText,
      language: 'en',
      durationMs: 12,
      audioDurationSeconds: 1,
    }));
    mocks.recorderStop.mockClear();
    mocks.finishInputSession.mockClear();
    mocks.cancelInputSession.mockClear();
    mocks.downloadModel.mockReset();
    mocks.downloadModel.mockImplementation(async () => ({
      modelId: 'sensevoice-test-model',
      displayName: 'SenseVoice test',
      provider: 'test',
      version: 'test',
      description: 'Test speech model',
      languages: ['auto', 'en'],
      state: 'installed',
      installedBytes: 165 * 1024 * 1024,
      expectedBytes: 165 * 1024 * 1024,
    }));
    mocks.cancelModelDownload.mockClear();
    mocks.modelStatusListener = undefined;
    mocks.notificationInfo.mockClear();
    mocks.notificationError.mockClear();
    focusInputSoon = vi.fn();
    insertText = vi.fn(() => 'Existing draft Transcribed request');
    submitText = vi.fn(async () => undefined);
    controller = undefined;
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: { getUserMedia: vi.fn() },
    });
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);

    await act(async () => {
      root.render(
        <Probe
          focusInputSoon={focusInputSoon}
          insertText={insertText}
          submitText={submitText}
          onController={(next) => { controller = next; }}
        />,
      );
      await Promise.resolve();
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  async function startRecording() {
    await act(async () => {
      controller?.toggle();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(controller?.phase).toBe('recording');
  }

  it('inserts the transcript without sending in transcribe-only mode', async () => {
    await startRecording();

    await act(async () => {
      controller?.transcribe();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(insertText).toHaveBeenCalledWith('Transcribed request');
    expect(focusInputSoon).toHaveBeenCalledOnce();
    expect(submitText).not.toHaveBeenCalled();
  });

  it('submits the merged draft in transcribe-and-send mode', async () => {
    await startRecording();

    await act(async () => {
      controller?.transcribeAndSend();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(insertText).toHaveBeenCalledWith('Transcribed request');
    expect(submitText).toHaveBeenCalledWith('Existing draft Transcribed request');
    expect(focusInputSoon).not.toHaveBeenCalled();
  });

  it('does not submit the existing draft when recognition is empty', async () => {
    mocks.finishText = '   ';
    await startRecording();

    await act(async () => {
      controller?.transcribeAndSend();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(insertText).not.toHaveBeenCalled();
    expect(submitText).not.toHaveBeenCalled();
    expect(mocks.notificationInfo).toHaveBeenCalledOnce();
  });

  it('keeps the idle control actionable when microphone capture is unavailable', async () => {
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: undefined,
    });
    await act(async () => {
      root.render(
        <Probe
          focusInputSoon={focusInputSoon}
          insertText={insertText}
          submitText={submitText}
          onController={(next) => { controller = next; }}
        />,
      );
      await Promise.resolve();
    });

    expect(controller?.disabled).toBe(false);
    await act(async () => {
      controller?.toggle();
      await Promise.resolve();
    });

    expect(controller?.phase).toBe('idle');
    expect(mocks.notificationError).toHaveBeenCalledWith('input.voiceInput.unsupported');
  });

  it('downloads a missing local model in place and continues into recording', async () => {
    await act(async () => {
      mocks.modelStatusListener?.({
        modelId: 'sensevoice-test-model',
        displayName: 'SenseVoice test',
        provider: 'test',
        version: 'test',
        description: 'Test speech model',
        languages: ['auto', 'en'],
        state: 'not_installed',
        installedBytes: 0,
        expectedBytes: 165 * 1024 * 1024,
      });
      await Promise.resolve();
    });

    await act(async () => {
      controller?.toggle();
      await Promise.resolve();
    });
    expect(controller?.phase).toBe('setup');
    expect(controller?.setupMessage).toBe('input.voiceInput.setupRequired');

    await act(async () => {
      controller?.installAndStart();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.downloadModel).toHaveBeenCalledWith('sensevoice-test-model');
    expect(controller?.phase).toBe('recording');
  });

  it('dismisses a stale setup prompt when the selected model becomes installed', async () => {
    await act(async () => {
      mocks.modelStatusListener?.(speechModel(
        'sensevoice-test-model',
        'not_installed',
        165 * 1024 * 1024,
      ));
      await Promise.resolve();
    });

    await act(async () => {
      controller?.toggle();
      await Promise.resolve();
    });
    expect(controller?.phase).toBe('setup');

    await act(async () => {
      mocks.modelStatusListener?.(speechModel(
        'sensevoice-test-model',
        'installed',
        165 * 1024 * 1024,
      ));
      await Promise.resolve();
    });

    expect(controller?.phase).toBe('idle');
  });

  it('ignores an old model query that completes after the selected model query', async () => {
    mocks.listModels.mockImplementationOnce(async () => ({
      models: [speechModel('qwen-test-model', 'installed', 838 * 1024 * 1024)],
    }));
    mocks.voiceInputSettings = {
      ...mocks.voiceInputSettings,
      model_id: 'qwen-test-model',
    };
    await act(async () => {
      root.render(
        <Probe
          focusInputSoon={focusInputSoon}
          insertText={insertText}
          submitText={submitText}
          onController={(next) => { controller = next; }}
        />,
      );
      await Promise.resolve();
    });

    const oldSenseVoiceQuery = createDeferred<{ models: ReturnType<typeof speechModel>[] }>();
    const currentQwenQuery = createDeferred<{ models: ReturnType<typeof speechModel>[] }>();
    mocks.listModels
      .mockImplementationOnce(() => oldSenseVoiceQuery.promise)
      .mockImplementationOnce(() => currentQwenQuery.promise);

    mocks.voiceInputSettings = {
      ...mocks.voiceInputSettings,
      model_id: 'sensevoice-test-model',
    };
    await act(async () => {
      root.render(
        <Probe
          focusInputSoon={focusInputSoon}
          insertText={insertText}
          submitText={submitText}
          onController={(next) => { controller = next; }}
        />,
      );
    });

    mocks.voiceInputSettings = {
      ...mocks.voiceInputSettings,
      model_id: 'qwen-test-model',
    };
    await act(async () => {
      root.render(
        <Probe
          focusInputSoon={focusInputSoon}
          insertText={insertText}
          submitText={submitText}
          onController={(next) => { controller = next; }}
        />,
      );
    });

    await act(async () => {
      currentQwenQuery.resolve({
        models: [speechModel('qwen-test-model', 'installed', 838 * 1024 * 1024)],
      });
      await currentQwenQuery.promise;
    });
    await act(async () => {
      oldSenseVoiceQuery.resolve({
        models: [speechModel('sensevoice-test-model', 'not_installed', 165 * 1024 * 1024)],
      });
      await oldSenseVoiceQuery.promise;
    });

    await startRecording();
    expect(controller?.phase).toBe('recording');
  });
});
