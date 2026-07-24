import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Activity, Mic, RefreshCw, Square } from 'lucide-react';
import { Button, IconButton, Select, type SelectOption } from '@/component-library';
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
import {
  ConfigPageRow,
  ConfigPageSection,
} from './common';

const log = createLogger('VoiceInputDiagnostics');
const TEST_CHUNK_DURATION_MS = 500;
const MICROPHONE_TEST_LIMIT_MS = 8000;
const RECOGNITION_TEST_LIMIT_MS = 15000;

type DiagnosticPhase =
  | 'idle'
  | 'preparing-microphone'
  | 'checking-microphone'
  | 'preparing-recognition'
  | 'recording'
  | 'transcribing';

interface VoiceInputDiagnosticsProps {
  settings: VoiceInputSettings;
  modelInstalled: boolean;
  onDeviceChange: (deviceId: string) => Promise<void>;
}

function normalizeSelectValue(value: string | number | (string | number)[]): string {
  return String(Array.isArray(value) ? (value[0] ?? '') : value);
}

export function VoiceInputDiagnostics({
  settings,
  modelInstalled,
  onDeviceChange,
}: VoiceInputDiagnosticsProps) {
  const { t } = useTranslation('settings/voice-input');
  const [microphones, setMicrophones] = useState<VoiceInputMicrophone[]>([]);
  const [devicesLoading, setDevicesLoading] = useState(false);
  const [phase, setPhase] = useState<DiagnosticPhase>('idle');
  const [level, setLevel] = useState(0);
  const [result, setResult] = useState<SpeechTranscriptionResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const recorderRef = useRef<VoiceInputRecorder | null>(null);
  const sessionRef = useRef<SpeechInputSession | null>(null);
  const pendingAppendRef = useRef<Promise<void>>(Promise.resolve());
  const timerRef = useRef<number | null>(null);
  const mountedRef = useRef(true);
  const activeCaptureIdRef = useRef(0);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const loadMicrophones = useCallback(async () => {
    setDevicesLoading(true);
    try {
      const devices = await listVoiceInputMicrophones();
      if (mountedRef.current) setMicrophones(devices);
    } catch (loadError) {
      log.warn('Failed to enumerate voice input microphones', { error: loadError });
    } finally {
      if (mountedRef.current) setDevicesLoading(false);
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

  const startMicrophoneTest = useCallback(async () => {
    const captureId = activeCaptureIdRef.current + 1;
    activeCaptureIdRef.current = captureId;
    setError(null);
    setResult(null);
    setLevel(0);
    setPhase('preparing-microphone');
    try {
      const recorder = await createVoiceInputRecorder({
        targetSampleRate: DEFAULT_SPEECH_SAMPLE_RATE,
        chunkDurationMs: TEST_CHUNK_DURATION_MS,
        microphoneDeviceId: settings.microphone_device_id || undefined,
        onChunk: () => undefined,
        onLevel: nextLevel => setLevel(nextLevel),
        onDeviceEnded: handleDeviceEnded,
      });
      if (!mountedRef.current || activeCaptureIdRef.current !== captureId) {
        await recorder.stop().catch(() => undefined);
        return;
      }
      recorderRef.current = recorder;
      setPhase('checking-microphone');
      await loadMicrophones();
      if (activeCaptureIdRef.current !== captureId) return;
      timerRef.current = window.setTimeout(() => {
        void resetCapture(false);
      }, MICROPHONE_TEST_LIMIT_MS);
    } catch (testError) {
      log.warn('Failed to start microphone diagnostic', { error: testError });
      setError(t('diagnostics.messages.microphoneFailed'));
      await resetCapture(true);
    }
  }, [handleDeviceEnded, loadMicrophones, resetCapture, settings.microphone_device_id, t]);

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
      setResult(transcription);
      setError(transcription.text.trim() ? null : t('diagnostics.messages.noSpeech'));
    } catch (testError) {
      log.error('Voice input recognition diagnostic failed', { error: testError });
      setError(t('diagnostics.messages.recognitionFailed'));
      await speechAPI.cancelInputSession(session.sessionId).catch(() => undefined);
    } finally {
      sessionRef.current = null;
      pendingAppendRef.current = Promise.resolve();
      setLevel(0);
      setPhase('idle');
    }
  }, [clearTimer, t]);

  const startRecognitionTest = useCallback(async () => {
    const captureId = activeCaptureIdRef.current + 1;
    activeCaptureIdRef.current = captureId;
    setError(null);
    setResult(null);
    setLevel(0);
    setPhase('preparing-recognition');
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
      setError(t('diagnostics.messages.recognitionFailed'));
      if (startedSession && sessionRef.current?.sessionId !== startedSession.sessionId) {
        await speechAPI.cancelInputSession(startedSession.sessionId).catch(cancelError => {
          log.warn('Failed to cancel voice input recognition session after startup failure', {
            sessionId: startedSession?.sessionId,
            error: cancelError,
          });
        });
      }
      await resetCapture(true);
    }
  }, [finishRecognitionTest, handleDeviceEnded, loadMicrophones, resetCapture, settings, t]);

  const preparingMicrophone = phase === 'preparing-microphone';
  const testingMicrophone = preparingMicrophone || phase === 'checking-microphone';
  const testingRecognition = phase === 'preparing-recognition' || phase === 'recording' || phase === 'transcribing';
  const volumeState = level < 0.01 ? 'silent' : level < 0.08 ? 'low' : 'normal';

  return (
    <ConfigPageSection title={t('sections.diagnostics')}>
      <ConfigPageRow
        label={t('diagnostics.microphone.label')}
        description={t('diagnostics.microphone.description')}
        align="center"
      >
        <div className="voice-input-config__device-control">
          <Select
            value={settings.microphone_device_id}
            onChange={value => void onDeviceChange(normalizeSelectValue(value))}
            options={microphoneOptions}
            size="small"
            loading={devicesLoading}
            className="voice-input-config__device-select"
          />
          <IconButton
            size="small"
            variant="ghost"
            aria-label={t('diagnostics.microphone.refresh')}
            tooltip={t('diagnostics.microphone.refresh')}
            disabled={phase !== 'idle'}
            onClick={() => void loadMicrophones()}
          >
            <RefreshCw size={14} />
          </IconButton>
        </div>
      </ConfigPageRow>

      <ConfigPageRow
        label={t('diagnostics.level.label')}
        description={t('diagnostics.level.description')}
        align="center"
      >
        <div className="voice-input-config__diagnostic-action">
          <div className="voice-input-config__level" aria-hidden="true">
            <div
              className={`voice-input-config__level-value voice-input-config__level-value--${volumeState}`}
              style={{ transform: `scaleX(${Math.max(0.02, level)})` }}
            />
          </div>
          <Button
            variant={testingMicrophone ? 'secondary' : 'ghost'}
            size="small"
            isLoading={preparingMicrophone}
            disabled={testingRecognition || preparingMicrophone}
            onClick={() => {
              if (phase === 'checking-microphone') void resetCapture(false);
              else void startMicrophoneTest();
            }}
          >
            {testingMicrophone ? <Square size={13} /> : <Mic size={14} />}
            {testingMicrophone ? t('diagnostics.level.stop') : t('diagnostics.level.start')}
          </Button>
        </div>
      </ConfigPageRow>

      <ConfigPageRow
        label={t('diagnostics.recognition.label')}
        description={t('diagnostics.recognition.description')}
        align="start"
      >
        <div className="voice-input-config__recognition-test">
          <Button
            variant={phase === 'recording' ? 'secondary' : 'primary'}
            size="small"
            isLoading={phase === 'preparing-recognition' || phase === 'transcribing'}
            disabled={testingMicrophone || (!modelInstalled && phase === 'idle')}
            onClick={() => {
              if (phase === 'recording') void finishRecognitionTest();
              else if (phase === 'idle') void startRecognitionTest();
            }}
          >
            {phase === 'recording' ? <Square size={13} /> : <Activity size={14} />}
            {phase === 'recording'
              ? t('diagnostics.recognition.finish')
              : t('diagnostics.recognition.start')}
          </Button>
          {!modelInstalled ? (
            <span className="voice-input-config__diagnostic-note">
              {t('diagnostics.recognition.modelRequired')}
            </span>
          ) : null}
          {result?.text.trim() ? (
            <div className="voice-input-config__recognition-result">
              <span>{result.text.trim()}</span>
              <small>{t('diagnostics.recognition.timing', { duration: result.durationMs })}</small>
            </div>
          ) : null}
          {error ? <span className="voice-input-config__diagnostic-error">{error}</span> : null}
        </div>
      </ConfigPageRow>
    </ConfigPageSection>
  );
}
