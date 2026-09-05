import { Button, Select, type SelectOption } from '@openbitfun/ui';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Square } from 'lucide-react';

import {
  DEFAULT_SPEECH_SAMPLE_RATE,
  speechAPI,
  type SpeechInputSession,
  type SpeechTranscriptionResult,
} from '@/infrastructure/api';
import {
  createVoiceInputRecorder,
  listVoiceInputMicrophones,
  type VoiceInputMicrophone,
  type VoiceInputRecorder,
} from '@/infrastructure/speech/voiceInputAudio';
import { useTranslation } from 'react-i18next';
import { createLogger } from '@/shared/utils/logger';
import type { VoiceInputSettings } from '../types';
import { ConfigPageRow } from './common';

const log = createLogger('VoiceInputDiagnostics');
const TEST_CHUNK_DURATION_MS = 500;
const RECOGNITION_TEST_LIMIT_MS = 15000;
const WAVEFORM_BAR_WEIGHTS = [0.52, 0.78, 1, 0.68, 0.9, 0.62, 0.44];

type DiagnosticPhase = 'idle' | 'preparing' | 'recording' | 'transcribing';

interface VoiceInputDiagnosticsProps {
  settings: VoiceInputSettings;
  onDeviceChange: (deviceId: string) => Promise<void>;
}

export function VoiceInputDiagnostics({
  settings,
  onDeviceChange,
}: VoiceInputDiagnosticsProps) {
  const { t } = useTranslation('settings/voice-input');
  const [microphones, setMicrophones] = useState<VoiceInputMicrophone[]>([]);
  const [phase, setPhase] = useState<DiagnosticPhase>('idle');
  const [level, setLevel] = useState(0);
  const [result, setResult] = useState<SpeechTranscriptionResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const recorderRef = useRef<VoiceInputRecorder | null>(null);
  const sessionRef = useRef<SpeechInputSession | null>(null);
  const pendingAppendRef = useRef<Promise<void>>(Promise.resolve());
  const timerRef = useRef<number | null>(null);
  const mountedRef = useRef(true);
  const microphoneLoadIdRef = useRef(0);
  const activeCaptureIdRef = useRef(0);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const loadMicrophones = useCallback(async () => {
    const loadId = microphoneLoadIdRef.current + 1;
    microphoneLoadIdRef.current = loadId;
    try {
      const devices = await listVoiceInputMicrophones();
      if (mountedRef.current && microphoneLoadIdRef.current === loadId) {
        setMicrophones(devices);
      }
    } catch (loadError) {
      log.warn('Failed to enumerate voice input microphones', { error: loadError });
    }
  }, []);

  const resetCapture = useCallback(async (cancelSession: boolean) => {
    activeCaptureIdRef.current += 1;
    clearTimer();
    const recorder = recorderRef.current;
    const session = sessionRef.current;
    recorderRef.current = null;
    sessionRef.current = null;
    pendingAppendRef.current = Promise.resolve();
    if (recorder) {
      await recorder.stop().catch(stopError => {
        log.warn('Failed to stop voice input diagnostic recorder', { error: stopError });
      });
    }
    if (cancelSession && session) {
      await speechAPI.cancelInputSession(session.sessionId).catch(cancelError => {
        log.warn('Failed to cancel voice input diagnostic session', { error: cancelError });
      });
    }
    if (mountedRef.current) {
      setLevel(0);
      setPhase('idle');
    }
  }, [clearTimer]);

  useEffect(() => {
    mountedRef.current = true;
    void loadMicrophones();
    const handleDeviceChange = () => void loadMicrophones();
    navigator.mediaDevices?.addEventListener?.('devicechange', handleDeviceChange);
    return () => {
      mountedRef.current = false;
      navigator.mediaDevices?.removeEventListener?.('devicechange', handleDeviceChange);
      void resetCapture(true);
    };
  }, [loadMicrophones, resetCapture]);

  const microphoneOptions = useMemo<SelectOption[]>(() => [
    { label: t('diagnostics.microphone.systemDefault'), value: '' },
    ...microphones.map((microphone, index) => ({
      label: microphone.label || t('diagnostics.microphone.unnamed', { index: index + 1 }),
      value: microphone.deviceId,
    })),
  ], [microphones, t]);

  const handleDeviceEnded = useCallback(() => {
    setError(t('diagnostics.messages.deviceDisconnected'));
    void resetCapture(true);
  }, [resetCapture, t]);

  const finishRecognitionTest = useCallback(async () => {
    activeCaptureIdRef.current += 1;
    clearTimer();
    const recorder = recorderRef.current;
    const session = sessionRef.current;
    if (!recorder || !session) return;
    recorderRef.current = null;
    setPhase('transcribing');
    try {
      await recorder.stop();
      await pendingAppendRef.current;
      const transcription = await speechAPI.finishInputSession(session.sessionId);
      sessionRef.current = null;
      if (mountedRef.current) {
        setResult(transcription);
        setError(transcription.text.trim() ? null : t('diagnostics.messages.noSpeech'));
      }
    } catch (testError) {
      log.error('Voice input recognition diagnostic failed', { error: testError });
      if (mountedRef.current) setError(t('diagnostics.messages.recognitionFailed'));
      await speechAPI.cancelInputSession(session.sessionId).catch(() => undefined);
    } finally {
      sessionRef.current = null;
      pendingAppendRef.current = Promise.resolve();
      if (mountedRef.current) {
        setLevel(0);
        setPhase('idle');
      }
    }
  }, [clearTimer, t]);

  const startRecognitionTest = useCallback(async () => {
    const captureId = activeCaptureIdRef.current + 1;
    activeCaptureIdRef.current = captureId;
    setError(null);
    setResult(null);
    setLevel(0);
    setPhase('preparing');
    let startedSession: SpeechInputSession | null = null;
    try {
      const session = await speechAPI.startInputSession({
        modelId: settings.model_id,
        language: settings.default_language,
        sampleRate: DEFAULT_SPEECH_SAMPLE_RATE,
        maxRecordingSeconds: Math.min(settings.max_recording_seconds, 30),
      });
      startedSession = session;
      if (!mountedRef.current || activeCaptureIdRef.current !== captureId) {
        await speechAPI.cancelInputSession(session.sessionId).catch(() => undefined);
        return;
      }
      sessionRef.current = session;
      pendingAppendRef.current = Promise.resolve();
      const recorder = await createVoiceInputRecorder({
        targetSampleRate: DEFAULT_SPEECH_SAMPLE_RATE,
        chunkDurationMs: TEST_CHUNK_DURATION_MS,
        microphoneDeviceId: settings.microphone_device_id || undefined,
        onChunk: pcm16Base64 => {
          pendingAppendRef.current = pendingAppendRef.current.then(async () => {
            await speechAPI.appendAudioChunk(session.sessionId, pcm16Base64);
          });
        },
        onLevel: nextLevel => setLevel(nextLevel),
        onDeviceEnded: handleDeviceEnded,
      });
      if (!mountedRef.current || activeCaptureIdRef.current !== captureId) {
        await recorder.stop().catch(() => undefined);
        await speechAPI.cancelInputSession(session.sessionId).catch(() => undefined);
        return;
      }
      recorderRef.current = recorder;
      setPhase('recording');
      await loadMicrophones();
      if (activeCaptureIdRef.current !== captureId) return;
      timerRef.current = window.setTimeout(() => {
        void finishRecognitionTest();
      }, RECOGNITION_TEST_LIMIT_MS);
    } catch (testError) {
      log.error('Failed to start voice input recognition diagnostic', { error: testError });
      if (mountedRef.current) setError(t('diagnostics.messages.recognitionFailed'));
      if (startedSession) {
        await speechAPI.cancelInputSession(startedSession.sessionId).catch(cancelError => {
          log.warn('Failed to cancel voice input diagnostic session after startup failure', {
            sessionId: startedSession?.sessionId,
            error: cancelError,
          });
        });
      }
      await resetCapture(false);
    }
  }, [finishRecognitionTest, handleDeviceEnded, loadMicrophones, resetCapture, settings, t]);

  const testingRecognition = phase !== 'idle';

  return (
    <div
      className="voice-input-config__diagnostics"
      data-openbitfun-component="voice-input-diagnostics"
      data-openbitfun-part="root"
      data-openbitfun-phase={phase}
      data-openbitfun-state={[
        testingRecognition && 'testing-recognition',
        error && 'error',
      ].filter(Boolean).join(' ')}
    >
      <ConfigPageRow
        label={t('diagnostics.microphone.label')}
        description={t('diagnostics.microphone.description')}
        align="center"
      >
        <div className="voice-input-config__device-control" data-openbitfun-component="voice-input-diagnostics" data-openbitfun-part="deviceControl">
          <Select
            data-openbitfun-component="voice-input-diagnostics"
            data-openbitfun-part="deviceSelect"
            value={settings.microphone_device_id}
            onValueChange={value => void onDeviceChange(String(value))}
            onPointerDown={() => void loadMicrophones()}
            options={microphoneOptions}
            size="sm"
          />
        </div>
      </ConfigPageRow>

      <ConfigPageRow
        label={t('diagnostics.recognition.label')}
        description={t('diagnostics.recognition.description')}
        align="start"
        className="voice-input-config__balanced-row"
      >
        <div className="voice-input-config__diagnostic-action" data-openbitfun-component="voice-input-diagnostics" data-openbitfun-part="diagnosticAction">
          {phase === 'recording' ? (
            <div className="voice-input-config__waveform" aria-hidden="true">
              {WAVEFORM_BAR_WEIGHTS.map((weight, index) => (
                <span
                  key={index}
                  className="voice-input-config__waveform-bar"
                  style={{
                    transform: `scaleY(${Math.min(1, Math.max(0.16, level * 8 * weight))})`,
                  }}
                />
              ))}
            </div>
          ) : null}
          <Button
            className="voice-input-config__diagnostic-button"
            variant={phase === 'recording' ? 'outline' : 'fill'}
            size="sm"
            loading={phase === 'preparing' || phase === 'transcribing'}
            disabled={phase === 'preparing' || phase === 'transcribing'}
            leadingIcon={phase === 'recording' ? <Square size={13} /> : undefined}
            onClick={() => {
              if (phase === 'recording') void finishRecognitionTest();
              else if (phase === 'idle') void startRecognitionTest();
            }}
          >
            {phase === 'recording'
              ? t('diagnostics.recognition.finish')
              : phase === 'preparing'
                ? t('diagnostics.recognition.preparing')
                : phase === 'transcribing'
                  ? t('diagnostics.recognition.transcribing')
                  : t('diagnostics.recognition.start')}
          </Button>
        </div>
      </ConfigPageRow>

      <div
        className="voice-input-config__recognition-feedback"
        data-openbitfun-component="voice-input-diagnostics"
        data-openbitfun-part="feedback"
        aria-live="polite"
      >
        {result?.text.trim() ? (
          <div className="voice-input-config__recognition-result" data-openbitfun-component="voice-input-diagnostics" data-openbitfun-part="result">
            <span>{result.text.trim()}</span>
            <small>{t('diagnostics.recognition.timing', { duration: result.durationMs })}</small>
          </div>
        ) : null}
        {error ? (
          <span className="voice-input-config__diagnostic-error" data-openbitfun-component="voice-input-diagnostics" data-openbitfun-part="error">{error}</span>
        ) : null}
      </div>
    </div>
  );
}
