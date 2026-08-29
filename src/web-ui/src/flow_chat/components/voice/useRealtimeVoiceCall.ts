import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  DEFAULT_REALTIME_OUTPUT_SAMPLE_RATE,
  DEFAULT_SPEECH_SAMPLE_RATE,
  speechAPI,
  type SpeechRealtimeEvent,
  type SpeechRealtimeFunctionCall,
  type SpeechRealtimeConfig,
  type SpeechRealtimeSession,
} from '@/infrastructure/api';
import { isTauriRuntime } from '@/infrastructure/runtime';
import {
  createVoiceInputRecorder,
  type VoiceInputRecorder,
} from '@/infrastructure/speech/voiceInputAudio';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { RealtimePcmPlayer } from './realtimeVoiceAudio';
import { applyRealtimeAsrSnapshot } from './realtimeVoiceTranscript';
import {
  runBitFunVoiceTask,
  summarizeVoiceTaskConclusion,
  VoiceTaskCancelledError,
  type VoiceTaskProgress,
  type VoiceTaskProgressPhase,
  type VoiceTaskResult,
} from './voiceTaskBridge';
import {
  buildVoiceClientContext,
  resolveOpenedVoiceWorkspace,
  serializeVoiceClientContext,
  switchOpenedVoiceWorkspace,
  type VoiceOwnedTaskContext,
} from './voiceClientContext';

const log = createLogger('RealtimeVoiceCall');
const AUDIO_CHUNK_DURATION_MS = 20;
const PCM_BYTES_PER_SAMPLE = 2;
const MAX_TRANSCRIPT_CHARS = 1_200;
const TASK_STOP_TIMEOUT_MS = 35_000;
const MISSING_AUDIO_RETRY_DELAY_MS = 1_500;
const MAX_BUFFERED_STARTUP_EVENTS = 256;
const SPOKEN_PROGRESS_RETRY_DELAY_MS = 400;

export type RealtimeVoiceCallPhase = 'idle' | 'connecting' | 'live' | 'ending' | 'error';

export interface RealtimeVoiceCallController {
  enabled: boolean;
  disabled: boolean;
  phase: RealtimeVoiceCallPhase;
  muted: boolean;
  audioLevel: number;
  userTranscript: string;
  assistantTranscript: string;
  status: string;
  taskSessionId: string | null;
  taskPhase: VoiceTaskProgressPhase | null;
  taskProgressText: string;
  start: () => void;
  end: () => void;
  toggleMute: () => void;
  openSettings: () => void;
}

function appendTranscript(previous: string, next: string): string {
  const merged = `${previous}${next}`;
  return merged.length <= MAX_TRANSCRIPT_CHARS
    ? merged
    : merged.slice(merged.length - MAX_TRANSCRIPT_CHARS);
}

function silentPcm16Base64(sampleRate: number): string {
  const samples = Math.max(1, Math.floor(sampleRate * AUDIO_CHUNK_DURATION_MS / 1000));
  const bytes = new Uint8Array(samples * PCM_BYTES_PER_SAMPLE);
  let binary = '';
  for (let index = 0; index < bytes.length; index += 1) {
    binary += String.fromCharCode(bytes[index]);
  }
  return window.btoa(binary);
}

/**
 * Client control-plane commands exposed to the provider-hosted Voice model.
 *
 * This union mirrors the client tool schemas in
 * `src/crates/services/services-integrations/src/speech/realtime.rs`; it is not
 * the workspace Agent tool contract. `run_task` crosses that boundary by
 * delegating a complete request to a normal Agent session. The delegated Agent
 * independently resolves its filesystem, terminal, MCP, browser, and other
 * tools together with their usual permission policy.
 *
 * Extension rule for future agents:
 * - Add direct BitFun client operations here and in the Rust Voice schema.
 * - Add workspace execution abilities to the normal Agent tool registry; do
 *   not mirror individual Agent tools into Voice.
 * - Keep the parser, dispatcher, provider schema, and focused tests in sync.
 */
type VoiceFunctionCommand =
  | { kind: 'get_client_context' }
  | { kind: 'switch_workspace'; workspaceReference: string }
  | {
      kind: 'run_task';
      task: string;
      workspaceReference?: string;
      activateWorkspace: boolean;
    }
  | { kind: 'stop_task' };

type VoiceTaskOutcome =
  | { status: 'completed'; result: VoiceTaskResult }
  | { status: 'cancelled'; sessionId: string }
  | { status: 'failed'; error: string };

interface ActiveVoiceTask {
  callId: string;
  sessionId: string | null;
  workspaceId: string;
  workspaceName: string;
  state: 'starting' | 'running' | 'stopping';
  controller: AbortController;
  outcome: Promise<VoiceTaskOutcome>;
  resolveOutcome: (outcome: VoiceTaskOutcome) => void;
  settled: boolean;
}

function parseFunctionCall(call: SpeechRealtimeFunctionCall): VoiceFunctionCommand {
  if (call.name === 'get_bitfun_client_context') {
    return { kind: 'get_client_context' };
  }
  if (call.name === 'stop_bitfun_task') {
    return { kind: 'stop_task' };
  }
  const rawArguments = JSON.parse(call.arguments || '{}') as unknown;
  const parsed = (
    rawArguments && typeof rawArguments === 'object' ? rawArguments : {}
  ) as {
    task?: unknown;
    workspace_id?: unknown;
    activate_workspace?: unknown;
  };
  if (call.name === 'switch_bitfun_workspace') {
    if (typeof parsed.workspace_id !== 'string' || !parsed.workspace_id.trim()) {
      throw new Error('Workspace switch did not include an exact workspace_id');
    }
    return { kind: 'switch_workspace', workspaceReference: parsed.workspace_id.trim() };
  }
  if (call.name !== 'run_bitfun_task') {
    throw new Error(`Unsupported realtime voice function: ${call.name}`);
  }
  if (!parsed || typeof parsed.task !== 'string' || !parsed.task.trim()) {
    throw new Error('Realtime voice function did not include a task description');
  }
  return {
    kind: 'run_task',
    task: parsed.task.trim(),
    workspaceReference: typeof parsed.workspace_id === 'string'
      ? parsed.workspace_id.trim() || undefined
      : undefined,
    activateWorkspace: parsed.activate_workspace !== false,
  };
}

function activeTaskContext(task: ActiveVoiceTask | null): VoiceOwnedTaskContext | null {
  if (!task) return null;
  return {
    sessionId: task.sessionId,
    workspaceId: task.workspaceId,
    workspaceName: task.workspaceName,
    state: task.state,
  };
}

function settleActiveTask(task: ActiveVoiceTask, outcome: VoiceTaskOutcome): void {
  if (task.settled) return;
  task.settled = true;
  task.resolveOutcome(outcome);
}

async function waitForTaskOutcome(task: ActiveVoiceTask): Promise<VoiceTaskOutcome | null> {
  let timeoutId: number | null = null;
  const timeoutPromise = new Promise<null>(resolve => {
    timeoutId = window.setTimeout(() => resolve(null), TASK_STOP_TIMEOUT_MS);
  });
  try {
    return await Promise.race([task.outcome, timeoutPromise]);
  } finally {
    if (timeoutId !== null) window.clearTimeout(timeoutId);
  }
}

export function useRealtimeVoiceCallController(disabled = false): RealtimeVoiceCallController {
  const { t } = useTranslation('settings/voice-input');
  const [voiceCallConfig, setVoiceCallConfig] = useState<SpeechRealtimeConfig | null>(null);
  const [phase, setPhase] = useState<RealtimeVoiceCallPhase>('idle');
  const [muted, setMuted] = useState(false);
  const [audioLevel, setAudioLevel] = useState(0);
  const [userTranscript, setUserTranscript] = useState('');
  const [assistantTranscript, setAssistantTranscript] = useState('');
  const [status, setStatus] = useState('');
  const [taskSessionId, setTaskSessionId] = useState<string | null>(null);
  const [taskPhase, setTaskPhase] = useState<VoiceTaskProgressPhase | null>(null);
  const [taskProgressText, setTaskProgressText] = useState('');
  const sessionRef = useRef<SpeechRealtimeSession | null>(null);
  const recorderRef = useRef<VoiceInputRecorder | null>(null);
  const playerRef = useRef<RealtimePcmPlayer | null>(null);
  const mutedRef = useRef(false);
  const pendingAudioRef = useRef<Promise<void>>(Promise.resolve());
  const activeCallIdRef = useRef(0);
  const handledFunctionCallsRef = useRef(new Set<string>());
  const assistantTurnStartedRef = useRef(false);
  const assistantTextRef = useRef('');
  const assistantAudioResponseActiveRef = useRef(false);
  const assistantAudioBytesRef = useRef(0);
  const assistantSpeechFallbackSentRef = useRef(false);
  const assistantSpeechFallbackTimerRef = useRef<number | null>(null);
  const providerErrorRef = useRef(false);
  const activeTaskRef = useRef<ActiveVoiceTask | null>(null);
  const bufferStartupEventsRef = useRef(false);
  const startupEventsRef = useRef<SpeechRealtimeEvent[]>([]);
  const spokenProgressQueueRef = useRef<Promise<void>>(Promise.resolve());
  const spokenProgressEpochRef = useRef(0);

  useEffect(() => {
    mutedRef.current = muted;
  }, [muted]);

  const openSettings = useCallback(() => {
    useSettingsStore.getState().openDestination({
      pageId: 'application.input',
      viewId: 'voice',
    });
    useSceneStore.getState().openScene('settings');
  }, []);

  useEffect(() => {
    if (!isTauriRuntime()) return undefined;
    let active = true;
    const load = () => {
      void speechAPI.getRealtimeConfig().then(config => {
        if (active) setVoiceCallConfig(config);
      }).catch(error => {
        log.warn('Failed to load controller realtime voice settings', { error });
      });
    };
    const handleConfigChanged = (event: Event) => {
      const config = (event as CustomEvent<SpeechRealtimeConfig>).detail;
      if (config) setVoiceCallConfig(config);
      else load();
    };
    load();
    window.addEventListener('bitfun:realtime-voice-config-changed', handleConfigChanged);
    return () => {
      active = false;
      window.removeEventListener('bitfun:realtime-voice-config-changed', handleConfigChanged);
    };
  }, []);

  const progressText = useCallback((progress: VoiceTaskProgress): string => {
    return t(`voiceCall.call.taskPhases.${progress.phase}`);
  }, [t]);

  const enqueueSpokenTaskText = useCallback((sessionId: string, text: string): Promise<void> => {
    const spokenText = text.trim();
    if (!spokenText) return Promise.resolve();
    const epoch = spokenProgressEpochRef.current;
    const send = async () => {
      if (
        spokenProgressEpochRef.current !== epoch
        || sessionRef.current?.sessionId !== sessionId
      ) {
        return;
      }
      try {
        await speechAPI.speakRealtimeText(sessionId, spokenText);
      } catch (firstError) {
        log.warn('Failed to enqueue BitFun task speech; retrying once', {
          sessionId,
          firstError,
        });
        await new Promise<void>(resolve => {
          window.setTimeout(resolve, SPOKEN_PROGRESS_RETRY_DELAY_MS);
        });
        if (
          spokenProgressEpochRef.current !== epoch
          || sessionRef.current?.sessionId !== sessionId
        ) {
          return;
        }
        await speechAPI.speakRealtimeText(sessionId, spokenText);
      }
    };
    const queued = spokenProgressQueueRef.current
      .catch(() => undefined)
      .then(send);
    spokenProgressQueueRef.current = queued.catch(error => {
      log.warn('Failed to speak BitFun task update after retry', { sessionId, error });
      setStatus(t('voiceCall.call.status.audioPlaybackFailed'));
    });
    return queued;
  }, [t]);

  const speakProgress = useCallback((sessionId: string, progress: VoiceTaskProgress) => {
    const text = progressText(progress);
    setTaskPhase(progress.phase);
    setTaskProgressText(text);
    setStatus(text);
    void enqueueSpokenTaskText(sessionId, text);
  }, [enqueueSpokenTaskText, progressText]);

  const speakTextProgress = useCallback((sessionId: string, text: string) => {
    setTaskPhase(previous => previous === 'stopping' ? previous : 'working');
    setTaskProgressText(text);
    setStatus(text);
    void enqueueSpokenTaskText(sessionId, text);
  }, [enqueueSpokenTaskText]);

  const speakTaskOutcome = useCallback(async (
    sessionId: string,
    text: string,
  ): Promise<boolean> => {
    setTaskPhase(null);
    setTaskProgressText('');
    setAssistantTranscript(text);
    setStatus(text);
    try {
      // Share the progress queue so the closing brief cannot overtake an
      // already accepted in-flight update.
      await enqueueSpokenTaskText(sessionId, text);
      return true;
    } catch {
      return false;
    }
  }, [enqueueSpokenTaskText]);

  const clearAssistantSpeechFallbackTimer = useCallback(() => {
    if (assistantSpeechFallbackTimerRef.current === null) return;
    window.clearTimeout(assistantSpeechFallbackTimerRef.current);
    assistantSpeechFallbackTimerRef.current = null;
  }, []);

  const requestAssistantSpeechFallback = useCallback((sessionId: string, text: string) => {
    const spokenText = text.trim();
    if (!spokenText || assistantSpeechFallbackSentRef.current) return;
    assistantSpeechFallbackSentRef.current = true;
    setStatus(t('voiceCall.call.status.retryingAudio'));
    log.warn('Realtime response contained text but no audio; requesting provider TTS retry', {
      sessionId,
      textLength: spokenText.length,
    });
    void speechAPI.speakRealtimeText(sessionId, spokenText).catch(error => {
      log.error('Failed to request realtime assistant audio retry', { sessionId, error });
      setStatus(t('voiceCall.call.status.audioPlaybackFailed'));
    });
  }, [t]);

  const scheduleAssistantSpeechFallback = useCallback((sessionId: string) => {
    clearAssistantSpeechFallbackTimer();
    if (assistantAudioBytesRef.current > 0 || assistantSpeechFallbackSentRef.current) return;
    assistantSpeechFallbackTimerRef.current = window.setTimeout(() => {
      assistantSpeechFallbackTimerRef.current = null;
      if (assistantAudioBytesRef.current > 0) return;
      requestAssistantSpeechFallback(sessionId, assistantTextRef.current);
    }, MISSING_AUDIO_RETRY_DELAY_MS);
  }, [clearAssistantSpeechFallbackTimer, requestAssistantSpeechFallback]);

  const cleanupMedia = useCallback(async () => {
    clearAssistantSpeechFallbackTimer();
    const recorder = recorderRef.current;
    const player = playerRef.current;
    recorderRef.current = null;
    playerRef.current = null;
    if (recorder) await recorder.stop().catch(() => undefined);
    await pendingAudioRef.current.catch(() => undefined);
    if (player) await player.close().catch(() => undefined);
    setAudioLevel(0);
  }, [clearAssistantSpeechFallbackTimer]);

  /**
   * Dispatches Voice client commands only. It may read or mutate client state,
   * create one normal Agent session, or cancel the task owned by this call. It
   * must not become a second executor for workspace Agent tools; task-level
   * delegation and lifecycle events are the boundary between the two systems.
   */
  const handleFunctionCall = useCallback(async (
    callSessionId: string,
    call: SpeechRealtimeFunctionCall,
  ) => {
    if (handledFunctionCallsRef.current.has(call.callId)) return;
    handledFunctionCallsRef.current.add(call.callId);
    let command: VoiceFunctionCommand | null = null;
    try {
      command = parseFunctionCall(call);
      if (command.kind === 'get_client_context') {
        setStatus(t('voiceCall.call.status.readingClientContext'));
        await speechAPI.sendRealtimeToolResult(
          callSessionId,
          call.callId,
          JSON.stringify({
            ok: true,
            context: buildVoiceClientContext(activeTaskContext(activeTaskRef.current)),
          }),
        );
        setStatus(t('voiceCall.call.status.thinking'));
        return;
      }

      if (command.kind === 'switch_workspace') {
        setStatus(t('voiceCall.call.status.switchingWorkspace'));
        const workspace = await switchOpenedVoiceWorkspace(command.workspaceReference);
        await speechAPI.sendRealtimeToolResult(
          callSessionId,
          call.callId,
          JSON.stringify({
            ok: true,
            workspace: {
              id: workspace.id,
              name: workspace.name,
              path: workspace.rootPath,
              kind: workspace.workspaceKind,
            },
            context: buildVoiceClientContext(activeTaskContext(activeTaskRef.current)),
          }),
        );
        setStatus(t('voiceCall.call.status.listening'));
        return;
      }

      if (command.kind === 'stop_task') {
        const activeTask = activeTaskRef.current;
        if (!activeTask) {
          const text = t('voiceCall.call.noActiveTask');
          setTaskPhase(null);
          setTaskProgressText('');
          setStatus(text);
          await speechAPI.sendRealtimeToolResult(
            callSessionId,
            call.callId,
            JSON.stringify({ ok: true, cancelled: false, reason: 'no_active_task' }),
          );
          return;
        }

        const stoppingText = t('voiceCall.call.taskStopping');
        setTaskPhase('stopping');
        setTaskProgressText(stoppingText);
        setStatus(stoppingText);
        activeTask.state = 'stopping';
        activeTask.controller.abort();
        const outcome = await waitForTaskOutcome(activeTask);
        if (outcome?.status === 'cancelled') {
          const stoppedText = t('voiceCall.call.taskStopped');
          setTaskPhase(null);
          setTaskProgressText('');
          setStatus(stoppedText);
          await speechAPI.sendRealtimeToolResult(
            callSessionId,
            call.callId,
            JSON.stringify({
              ok: true,
              cancelled: true,
              session_id: outcome.sessionId,
            }),
          );
          return;
        }
        if (outcome?.status === 'completed') {
          const text = t('voiceCall.call.taskAlreadyComplete');
          setTaskPhase(null);
          setTaskProgressText('');
          setStatus(text);
          await speechAPI.sendRealtimeToolResult(
            callSessionId,
            call.callId,
            JSON.stringify({
              ok: true,
              cancelled: false,
              reason: 'already_completed',
              session_id: outcome.result.sessionId,
            }),
          );
          return;
        }
        if (outcome?.status === 'failed') {
          throw new Error(outcome.error);
        }
        await speechAPI.sendRealtimeToolResult(
          callSessionId,
          call.callId,
          JSON.stringify({
            ok: true,
            cancelled: false,
            cancel_requested: true,
            session_id: activeTask.sessionId,
          }),
        );
        return;
      }

      if (activeTaskRef.current) {
        await speechAPI.sendRealtimeToolResult(
          callSessionId,
          call.callId,
          JSON.stringify({
            ok: false,
            error: 'A BitFun task is already running through the client voice assistant',
            context: buildVoiceClientContext(activeTaskContext(activeTaskRef.current)),
          }),
        );
        return;
      }

      const workspace = resolveOpenedVoiceWorkspace(command.workspaceReference);
      if (command.activateWorkspace) {
        await switchOpenedVoiceWorkspace(workspace.id);
      }

      let resolveOutcome: (outcome: VoiceTaskOutcome) => void = () => undefined;
      const outcome = new Promise<VoiceTaskOutcome>(resolve => {
        resolveOutcome = resolve;
      });
      const activeTask: ActiveVoiceTask = {
        callId: call.callId,
        sessionId: null,
        workspaceId: workspace.id,
        workspaceName: workspace.name,
        state: 'starting',
        controller: new AbortController(),
        outcome,
        resolveOutcome,
        settled: false,
      };
      activeTaskRef.current = activeTask;
      setTaskPhase('starting');
      const startingText = t('voiceCall.call.taskPhases.starting');
      setTaskProgressText(startingText);
      setStatus(startingText);
      const result = await runBitFunVoiceTask(command.task, {
        workspace,
        showSession: command.activateWorkspace,
        signal: activeTask.controller.signal,
        onSessionCreated: sessionId => {
          activeTask.sessionId = sessionId;
          activeTask.state = 'running';
          setTaskSessionId(sessionId);
        },
        onProgress: progress => {
          const activeSpeechSessionId = sessionRef.current?.sessionId;
          if (activeSpeechSessionId) speakProgress(activeSpeechSessionId, progress);
        },
        onTextProgress: text => {
          const activeSpeechSessionId = sessionRef.current?.sessionId;
          if (activeSpeechSessionId) speakTextProgress(activeSpeechSessionId, text);
        },
      });
      settleActiveTask(activeTask, { status: 'completed', result });
      setTaskSessionId(result.sessionId);
      const outcomeText = result.conclusion
        ? t('voiceCall.call.taskOutcome.completed', { conclusion: result.conclusion })
        : t('voiceCall.call.taskOutcome.completedWithoutConclusion');
      const outcomeSpoken = await speakTaskOutcome(callSessionId, outcomeText);
      await speechAPI.sendRealtimeToolResult(
        callSessionId,
        call.callId,
        JSON.stringify({
          ok: true,
          session_id: result.sessionId,
          summary: result.summary,
          outcome_spoken: outcomeSpoken,
        }),
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const activeTask = activeTaskRef.current?.callId === call.callId
        ? activeTaskRef.current
        : null;
      if (activeTask && error instanceof VoiceTaskCancelledError) {
        settleActiveTask(activeTask, { status: 'cancelled', sessionId: error.sessionId });
        setTaskSessionId(error.sessionId);
        const outcomeSpoken = await speakTaskOutcome(
          callSessionId,
          t('voiceCall.call.taskOutcome.cancelled'),
        );
        await speechAPI.sendRealtimeToolResult(
          callSessionId,
          call.callId,
          JSON.stringify({
            ok: true,
            cancelled: true,
            session_id: error.sessionId,
            outcome_spoken: outcomeSpoken,
          }),
        ).catch(sendError => {
          log.warn('Failed to return BitFun task cancellation to realtime voice session', {
            sendError,
          });
        });
        return;
      }
      if (activeTask) {
        settleActiveTask(activeTask, { status: 'failed', error: message });
      }
      const isTaskCommand = command?.kind === 'run_task' || command?.kind === 'stop_task';
      let outcomeSpoken: boolean | undefined;
      if (isTaskCommand) {
        const reason = summarizeVoiceTaskConclusion(message);
        const outcomeText = reason
          ? t('voiceCall.call.taskOutcome.failed', { reason })
          : t('voiceCall.call.taskOutcome.failedWithoutReason');
        outcomeSpoken = await speakTaskOutcome(callSessionId, outcomeText);
      } else {
        setTaskPhase(null);
        setTaskProgressText('');
        setStatus(t('voiceCall.call.status.error'));
      }
      log.error('BitFun client voice tool failed', { callId: call.callId, tool: call.name, error });
      await speechAPI.sendRealtimeToolResult(
        callSessionId,
        call.callId,
        JSON.stringify({
          ok: false,
          error: message,
          ...(outcomeSpoken === undefined ? {} : { outcome_spoken: outcomeSpoken }),
        }),
      ).catch(sendError => {
        log.warn('Failed to return BitFun task error to realtime voice session', { sendError });
      });
    } finally {
      if (activeTaskRef.current?.callId === call.callId) {
        activeTaskRef.current = null;
      }
    }
  }, [speakProgress, speakTaskOutcome, speakTextProgress, t]);

  const handleRealtimeEvent = useCallback((event: SpeechRealtimeEvent) => {
    const session = sessionRef.current;
    if (!session) {
      // session.created is emitted before the Tauri start command can return
      // the local session id. Volcengine may begin proactive TTS immediately,
      // so retain those first PCM events instead of silently dropping them.
      if (
        bufferStartupEventsRef.current
        && startupEventsRef.current.length < MAX_BUFFERED_STARTUP_EVENTS
      ) {
        startupEventsRef.current.push(event);
      }
      return;
    }
    if (event.sessionId !== session.sessionId) return;

    switch (event.kind) {
      case 'connected':
        setStatus(t('voiceCall.call.status.connected'));
        break;
      case 'ready':
        setPhase('live');
        setStatus(t('voiceCall.call.status.listening'));
        break;
      case 'user_speech_started':
        clearAssistantSpeechFallbackTimer();
        playerRef.current?.stop();
        assistantTurnStartedRef.current = false;
        assistantTextRef.current = '';
        assistantAudioResponseActiveRef.current = false;
        assistantAudioBytesRef.current = 0;
        assistantSpeechFallbackSentRef.current = false;
        setUserTranscript('');
        setAssistantTranscript('');
        setStatus(t('voiceCall.call.status.listening'));
        void speechAPI.cancelRealtimeResponse(session.sessionId).catch(() => undefined);
        break;
      case 'user_transcript_delta':
        // Volcengine ASR deltas are revised partial snapshots (the official
        // demo prints each delta directly). Appending them produces strings
        // such as “current current project current project”. Display only the
        // newest partial snapshot; the completed event remains authoritative.
        if (event.text) {
          setUserTranscript(previous => applyRealtimeAsrSnapshot(previous, event.text!));
        }
        break;
      case 'user_transcript_completed':
        if (event.text) setUserTranscript(event.text);
        setStatus(t('voiceCall.call.status.thinking'));
        break;
      case 'assistant_text_delta':
        if (!assistantTurnStartedRef.current) {
          assistantTurnStartedRef.current = true;
          clearAssistantSpeechFallbackTimer();
          assistantTextRef.current = '';
          if (!assistantAudioResponseActiveRef.current) {
            assistantAudioBytesRef.current = 0;
          }
          assistantSpeechFallbackSentRef.current = false;
          setAssistantTranscript('');
        }
        if (event.text) {
          assistantTextRef.current = appendTranscript(assistantTextRef.current, event.text);
          setAssistantTranscript(previous => appendTranscript(previous, event.text!));
        }
        break;
      case 'assistant_text_completed':
        if (event.text) {
          assistantTextRef.current = event.text;
          setAssistantTranscript(event.text);
        }
        scheduleAssistantSpeechFallback(session.sessionId);
        break;
      case 'assistant_audio_started':
        clearAssistantSpeechFallbackTimer();
        assistantAudioResponseActiveRef.current = true;
        assistantAudioBytesRef.current = 0;
        playerRef.current?.stop();
        setStatus(t('voiceCall.call.status.speaking'));
        break;
      case 'assistant_audio_delta':
        if (event.audioBase64 && playerRef.current) {
          assistantAudioBytesRef.current += event.audioBase64.length;
          clearAssistantSpeechFallbackTimer();
          setStatus(t('voiceCall.call.status.speaking'));
          void playerRef.current.enqueue(event.audioBase64).catch(error => {
            log.error('Failed to play realtime assistant audio', { error });
            setStatus(t('voiceCall.call.status.audioPlaybackFailed'));
          });
        }
        break;
      case 'assistant_audio_completed':
        assistantAudioResponseActiveRef.current = false;
        assistantTurnStartedRef.current = false;
        if (assistantAudioBytesRef.current === 0) {
          scheduleAssistantSpeechFallback(session.sessionId);
        }
        if (!taskPhase) setStatus(t('voiceCall.call.status.listening'));
        break;
      case 'function_call':
        event.functionCalls?.forEach(call => {
          void handleFunctionCall(session.sessionId, call);
        });
        break;
      case 'error':
        providerErrorRef.current = true;
        setPhase('error');
        setStatus(event.message || t('voiceCall.call.status.error'));
        break;
      case 'closed':
        sessionRef.current = null;
        void cleanupMedia();
        if (phase === 'ending') {
          setPhase('idle');
        } else {
          setPhase('error');
          if (!providerErrorRef.current) {
            setStatus(t('voiceCall.call.status.disconnected'));
          }
        }
        break;
      default:
        break;
    }
  }, [
    cleanupMedia,
    clearAssistantSpeechFallbackTimer,
    handleFunctionCall,
    phase,
    scheduleAssistantSpeechFallback,
    t,
    taskPhase,
  ]);

  useEffect(() => speechAPI.onRealtimeEvent(handleRealtimeEvent), [handleRealtimeEvent]);

  const end = useCallback(() => {
    const session = sessionRef.current;
    if (!session && phase === 'idle') return;
    activeCallIdRef.current += 1;
    spokenProgressEpochRef.current += 1;
    bufferStartupEventsRef.current = false;
    startupEventsRef.current = [];
    setPhase('ending');
    setStatus(t('voiceCall.call.status.ending'));
    void cleanupMedia().finally(async () => {
      sessionRef.current = null;
      if (session) {
        await speechAPI.closeRealtimeSession(session.sessionId).catch(error => {
          log.warn('Failed to close realtime voice session', { sessionId: session.sessionId, error });
        });
      }
      setPhase('idle');
      setMuted(false);
      setUserTranscript('');
      setAssistantTranscript('');
      setTaskPhase(null);
      setTaskProgressText('');
      setTaskSessionId(null);
      setStatus('');
    });
  }, [cleanupMedia, phase, t]);

  const start = useCallback(() => {
    if (phase !== 'idle' || disabled) return;
    if (!isTauriRuntime()) {
      notificationService.error(t('messages.unsupported'));
      return;
    }
    const callId = activeCallIdRef.current + 1;
    activeCallIdRef.current = callId;
    spokenProgressEpochRef.current += 1;
    spokenProgressQueueRef.current = Promise.resolve();
    handledFunctionCallsRef.current.clear();
    clearAssistantSpeechFallbackTimer();
    bufferStartupEventsRef.current = true;
    startupEventsRef.current = [];
    assistantTextRef.current = '';
    assistantAudioResponseActiveRef.current = false;
    assistantAudioBytesRef.current = 0;
    assistantSpeechFallbackSentRef.current = false;
    providerErrorRef.current = false;
    const activeTask = activeTaskRef.current;
    setPhase('connecting');
    setMuted(false);
    setTaskSessionId(activeTask?.sessionId ?? null);
    setTaskPhase(
      activeTask?.state === 'starting'
        ? 'starting'
        : activeTask?.state === 'stopping'
          ? 'stopping'
          : activeTask
            ? 'working'
            : null,
    );
    setTaskProgressText(activeTask
      ? t(`voiceCall.call.taskPhases.${activeTask.state === 'running' ? 'working' : activeTask.state}`)
      : '');
    setUserTranscript('');
    setAssistantTranscript('');
    setStatus(t('voiceCall.call.status.connecting'));

    let preparedPlayer: RealtimePcmPlayer;
    try {
      // Construct the output context inside the click gesture. WebKit may keep
      // contexts created after the first network await suspended indefinitely.
      preparedPlayer = RealtimePcmPlayer.create(DEFAULT_REALTIME_OUTPUT_SAMPLE_RATE);
      playerRef.current = preparedPlayer;
    } catch (error) {
      bufferStartupEventsRef.current = false;
      startupEventsRef.current = [];
      log.error('Failed to prepare realtime audio output', { error });
      setPhase('error');
      setStatus(t('voiceCall.call.status.audioPlaybackFailed'));
      notificationService.error(t('voiceCall.call.startFailed'));
      return;
    }

    void (async () => {
      try {
        const controllerConfig = await speechAPI.getRealtimeConfig();
        setVoiceCallConfig(controllerConfig);
        if (!controllerConfig.enabled || !controllerConfig.apiKey.trim()) {
          bufferStartupEventsRef.current = false;
          startupEventsRef.current = [];
          await cleanupMedia();
          setPhase('idle');
          setStatus('');
          notificationService.info(t('voiceCall.call.configureFirst'));
          openSettings();
          return;
        }
        const session = await speechAPI.startRealtimeSession(
          serializeVoiceClientContext(activeTaskContext(activeTaskRef.current)),
        );
        if (activeCallIdRef.current !== callId) {
          bufferStartupEventsRef.current = false;
          startupEventsRef.current = [];
          await speechAPI.closeRealtimeSession(session.sessionId).catch(() => undefined);
          return;
        }
        sessionRef.current = session;
        preparedPlayer.setSourceSampleRate(session.outputSampleRate);
        bufferStartupEventsRef.current = false;
        const startupEvents = startupEventsRef.current;
        startupEventsRef.current = [];
        startupEvents
          .filter(event => event.sessionId === session.sessionId)
          .forEach(handleRealtimeEvent);
        if (
          providerErrorRef.current
          || sessionRef.current?.sessionId !== session.sessionId
        ) {
          throw new Error('Realtime voice session closed during audio startup');
        }
        const silence = silentPcm16Base64(session.inputSampleRate);
        recorderRef.current = await createVoiceInputRecorder({
          targetSampleRate: session.inputSampleRate || DEFAULT_SPEECH_SAMPLE_RATE,
          chunkDurationMs: AUDIO_CHUNK_DURATION_MS,
          audioContext: preparedPlayer.getAudioContext(),
          microphoneDeviceId: controllerConfig.microphoneDeviceId || undefined,
          onLevel: level => setAudioLevel(Math.max(0, Math.min(1, level))),
          onDeviceEnded: () => {
            if (sessionRef.current?.sessionId !== session.sessionId) return;
            activeCallIdRef.current += 1;
            sessionRef.current = null;
            setPhase('error');
            setStatus(t('voiceCall.call.status.microphoneDisconnected'));
            void cleanupMedia().finally(() => {
              void speechAPI.closeRealtimeSession(session.sessionId).catch(() => undefined);
            });
          },
          onChunk: pcm16Base64 => {
            const activeSession = sessionRef.current;
            if (!activeSession || activeSession.sessionId !== session.sessionId) return;
            const audio = mutedRef.current ? silence : pcm16Base64;
            pendingAudioRef.current = pendingAudioRef.current
              .catch(() => undefined)
              .then(() => speechAPI.appendRealtimeAudio(session.sessionId, audio))
              .catch(error => {
                log.warn('Failed to stream realtime microphone audio', {
                  sessionId: session.sessionId,
                  error,
                });
              });
          },
        });
        if (activeCallIdRef.current !== callId) {
          await cleanupMedia();
          return;
        }
        setPhase('live');
        setStatus(t('voiceCall.call.status.listening'));
      } catch (error) {
        if (activeCallIdRef.current !== callId) return;
        bufferStartupEventsRef.current = false;
        startupEventsRef.current = [];
        const failedSession = sessionRef.current;
        sessionRef.current = null;
        const message = error instanceof Error ? error.message : String(error);
        log.error('Failed to start realtime voice call', { error });
        setPhase('error');
        setStatus(message);
        notificationService.error(t('voiceCall.call.startFailed'));
        await cleanupMedia();
        if (failedSession) {
          await speechAPI.closeRealtimeSession(failedSession.sessionId).catch(() => undefined);
        }
      }
    })();
  }, [
    cleanupMedia,
    clearAssistantSpeechFallbackTimer,
    disabled,
    handleRealtimeEvent,
    openSettings,
    phase,
    t,
  ]);

  useEffect(() => () => {
    activeCallIdRef.current += 1;
    spokenProgressEpochRef.current += 1;
    bufferStartupEventsRef.current = false;
    startupEventsRef.current = [];
    const session = sessionRef.current;
    sessionRef.current = null;
    void cleanupMedia();
    if (session) void speechAPI.closeRealtimeSession(session.sessionId).catch(() => undefined);
  }, [cleanupMedia]);

  return {
    enabled: voiceCallConfig?.enabled !== false,
    disabled,
    phase,
    muted,
    audioLevel,
    userTranscript,
    assistantTranscript,
    status,
    taskSessionId,
    taskPhase,
    taskProgressText,
    start,
    end,
    toggleMute: () => setMuted(previous => !previous),
    openSettings,
  };
}
