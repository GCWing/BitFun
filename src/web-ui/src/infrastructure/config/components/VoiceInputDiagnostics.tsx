import { Icon as CatalogIcon, Button, IconButton, Select, type SelectOption, Tooltip } from '@bitfun/ui';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Activity, Square } from 'lucide-react';

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

type DiagnosticPhase = 'idle' | 'preparing' | 'recording' | 'transcribing';

interface VoiceInputDiagnosticsProps {
  settings: VoiceInputSettings;
  modelInstalled: boolean;
  unavailableReason?: string;
  onDeviceChange: (deviceId: string) => Promise<void>;
}

export function VoiceInputDiagnostics({
  settings,
  modelInstalled,
  unavailableReason,
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
  const volumeState = level < 0.01 ? 'silent' : level < 0.08 ? 'low' : 'normal';
  const recognitionUnavailableReason = unavailableReason
    ?? (!modelInstalled ? t('diagnostics.recognition.modelRequired') : undefined);

  return (
    <div
      className="voice-input-config__diagnostics"
      data-bf-component="voice-input-diagnostics"
      data-bf-part="root"
      data-bf-phase={phase}
      data-bf-state={[
        testingRecognition && 'testing-recognition',
        error && 'error',
      ].filter(Boolean).join(' ')}
    >
      <ConfigPageRow
        label={t('diagnostics.microphone.label')}
        description={t('diagnostics.microphone.description')}
        align="center"
        className="voice-input-config__diagnostic-row"
      >
        <div className="voice-input-config__device-control" data-bf-component="voice-input-diagnostics" data-bf-part="deviceControl">
          <Select
            data-bf-component="voice-input-diagnostics"
            data-bf-part="deviceSelect"
            value={settings.microphone_device_id}
            onValueChange={value => void onDeviceChange(String(value))}
            options={microphoneOptions}
            size="sm"
            disabled={devicesLoading}
            className="voice-input-config__device-select"
          />
          <Tooltip content={t('diagnostics.microphone.refresh')} disabled={phase !== 'idle'}>
            <IconButton
              size="sm"
              aria-label={t('diagnostics.microphone.refresh')}
              disabled={phase !== 'idle'}
              icon={<CatalogIcon name="refresh" size="lg" />}
              onClick={() => void loadMicrophones()}
            />
          </Tooltip>
        </div>
      </ConfigPageRow>

      <ConfigPageRow
        label={t('diagnostics.recognition.label')}
        description={t('diagnostics.recognition.description')}
        align="start"
        className="voice-input-config__diagnostic-row"
      >
        <div className="voice-input-config__recognition-test" data-bf-component="voice-input-diagnostics" data-bf-part="recognitionTest">
          <div className="voice-input-config__diagnostic-action" data-bf-component="voice-input-diagnostics" data-bf-part="diagnosticAction">
            <div className="voice-input-config__level" data-bf-component="voice-input-diagnostics" data-bf-part="level" aria-hidden="true">
              <div
                data-bf-component="voice-input-diagnostics"
                data-bf-part="levelValue"
                data-bf-volume={volumeState}
                className={`voice-input-config__level-value voice-input-config__level-value--${volumeState}`}
                style={{ transform: `scaleX(${Math.max(0.02, level)})` }}
              />
            </div>
            <Button
              className="voice-input-config__diagnostic-button"
              variant={phase === 'recording' ? 'outline' : 'fill'}
              size="sm"
              loading={phase === 'preparing' || phase === 'transcribing'}
              disabled={!modelInstalled && phase === 'idle'}
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
          </div>
          {recognitionUnavailableReason ? (
            <span className="voice-input-config__diagnostic-note" data-bf-component="voice-input-diagnostics" data-bf-part="note">
              {recognitionUnavailableReason}
            </span>
          ) : null}
          {result?.text.trim() ? (
            <div className="voice-input-config__recognition-result" data-bf-component="voice-input-diagnostics" data-bf-part="result">
              <span>{result.text.trim()}</span>
              <small>{t('diagnostics.recognition.timing', { duration: result.durationMs })}</small>
            </div>
          ) : null}
          {error ? (
            <span className="voice-input-config__diagnostic-error" data-bf-component="voice-input-diagnostics" data-bf-part="error">{error}</span>
          ) : null}
        </div>
      </ConfigPageRow>
    </div>
  );
}
