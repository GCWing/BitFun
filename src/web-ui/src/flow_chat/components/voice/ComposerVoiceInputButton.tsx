import { Button, IconButton } from '@bitfun/ui';
import { useEffect, useRef, useState } from 'react';
import { ArrowUp, Check, Download, Loader2, Mic, VolumeX, X } from 'lucide-react';
import { Tooltip } from '@bitfun/ui';
import type { ComposerVoiceInputController } from './useComposerVoiceInput';

const VOICE_TIMELINE_SAMPLE_COUNT = 32;
const VOICE_TIMELINE_TICK_MS = 86;
const VOICE_SILENCE_THRESHOLD = 0.035;

function createFlatTimelineSamples(): number[] {
  return Array.from({ length: VOICE_TIMELINE_SAMPLE_COUNT }, () => 0);
}

function formatElapsedTime(totalSeconds: number): string {
  const minutes = Math.floor(totalSeconds / 60).toString().padStart(2, '0');
  const seconds = (totalSeconds % 60).toString().padStart(2, '0');
  return `${minutes}:${seconds}`;
}

interface ComposerVoiceInputButtonProps {
  controller: ComposerVoiceInputController;
}

export function ComposerVoiceInputButton({ controller }: ComposerVoiceInputButtonProps) {
  const [timelineSamples, setTimelineSamples] = useState(createFlatTimelineSamples);
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const currentLevelRef = useRef(0);

  useEffect(() => {
    currentLevelRef.current = controller.audioLevel;
  }, [controller.audioLevel]);

  useEffect(() => {
    if (
      controller.phase === 'idle'
      || controller.phase === 'setup'
      || controller.phase === 'downloading'
      || controller.phase === 'preparing'
    ) {
      setTimelineSamples(createFlatTimelineSamples());
      return undefined;
    }
    if (controller.phase === 'transcribing') {
      return undefined;
    }

    setTimelineSamples(createFlatTimelineSamples());
    const timerId = window.setInterval(() => {
      const level = currentLevelRef.current < VOICE_SILENCE_THRESHOLD
        ? 0
        : Math.min(1, currentLevelRef.current);
      setTimelineSamples(previous => [...previous.slice(1), level]);
    }, VOICE_TIMELINE_TICK_MS);

    return () => window.clearInterval(timerId);
  }, [controller.phase]);

  useEffect(() => {
    if (
      controller.phase === 'idle'
      || controller.phase === 'setup'
      || controller.phase === 'downloading'
      || controller.phase === 'preparing'
    ) {
      setElapsedSeconds(0);
      return undefined;
    }
    if (controller.phase !== 'recording') {
      return undefined;
    }

    const timerId = window.setInterval(() => {
      setElapsedSeconds(previous => previous + 1);
    }, 1000);
    return () => window.clearInterval(timerId);
  }, [controller.phase]);

  if (!controller.enabled) {
    return null;
  }

  const setupRequired = controller.phase === 'setup';
  const downloading = controller.phase === 'downloading';

  if (setupRequired || downloading) {
    const progress = Math.min(100, Math.max(0, controller.downloadProgress ?? 0));
    return (
      <span
        className="bitfun-chat-input__voice-cluster bitfun-chat-input__voice-cluster--setup"
        data-bf-component="composer-voice-input"
        data-bf-part="root"
        data-bf-phase={controller.phase}
        data-bf-state="active"
      >
        <span
          className="bitfun-chat-input__voice-setup-pill"
          data-bf-component="composer-voice-input"
          data-bf-part="setupPill"
          role="group"
          aria-label={controller.setupMessage}
          aria-busy={downloading}
        >
          <span
            className="bitfun-chat-input__voice-setup-icon"
            data-bf-component="composer-voice-input"
            data-bf-part="status"
            aria-hidden="true"
          >
            {downloading
              ? <Loader2 size={14} className="bitfun-chat-input__voice-spinner" />
              : <Download size={14} />}
          </span>
          <span
            className="bitfun-chat-input__voice-setup-copy"
            data-bf-component="composer-voice-input"
            data-bf-part="setupMessage"
          >
            {controller.setupMessage}
            {downloading ? (
              <span className="bitfun-chat-input__voice-setup-progress" aria-hidden="true">
                <span style={{ width: `${progress}%` }} />
              </span>
            ) : null}
          </span>
          {setupRequired ? (
            <span data-bf-component="composer-voice-input" data-bf-part="action" data-bf-action="install">
              <Button
                className="bitfun-chat-input__voice-setup-action"
                variant="fill"
                size="sm"
                onClick={(event) => {
                  event.stopPropagation();
                  controller.installAndStart();
                }}
              >
                {controller.setupActionLabel}
              </Button>
            </span>
          ) : null}
          <span data-bf-component="composer-voice-input" data-bf-part="action" data-bf-action="dismiss">
            <Tooltip content={controller.setupCancelTooltip}>
              <IconButton
                aria-label={controller.setupCancelTooltip}
                className="bitfun-chat-input__voice-setup-dismiss"
                size="sm"
                onClick={(event) => {
                  event.stopPropagation();
                  controller.dismissSetup();
                }}
                icon={<X size={14} />}
              />
            </Tooltip>
          </span>
        </span>
      </span>
    );
  }

  const preparing = controller.phase === 'preparing';
  const transcribing = controller.phase === 'transcribing';
  const recording = controller.phase === 'recording';
  const activeVoicePill = preparing || recording || transcribing;

  if (activeVoicePill) {
    const currentSample = !recording || controller.audioLevel < VOICE_SILENCE_THRESHOLD
      ? 0
      : Math.min(1, controller.audioLevel);
    const visibleTimelineSamples = recording
      ? [...timelineSamples.slice(0, -1), currentSample]
      : timelineSamples;
    const controlsDisabled = preparing || transcribing;

    return (
      <span
        className="bitfun-chat-input__voice-cluster bitfun-chat-input__voice-cluster--recording"
        data-bf-component="composer-voice-input"
        data-bf-part="root"
        data-bf-phase={controller.phase}
        data-bf-state={['active', controller.lowVolumeWarning && 'low-volume'].filter(Boolean).join(' ')}
      >
        <span
          aria-label={controller.tooltip}
          aria-busy={preparing || transcribing}
          className="bitfun-chat-input__voice-pill"
          data-bf-component="composer-voice-input"
          data-bf-part="pill"
          role="group"
        >
          <span
            className="bitfun-chat-input__voice-pill-status"
            data-bf-component="composer-voice-input"
            data-bf-part="status"
            title={controller.lowVolumeWarning ? controller.lowVolumeTooltip : undefined}
            aria-hidden="true"
          >
            {preparing ? (
              <Loader2 size={12} className="bitfun-chat-input__voice-spinner" />
            ) : controller.lowVolumeWarning ? (
              <VolumeX
                size={13}
                className="bitfun-chat-input__voice-low-volume"
              />
            ) : (
              <span className="bitfun-chat-input__voice-pill-recording-dot" />
            )}
          </span>

          <span className="bitfun-chat-input__voice-pill-time" data-bf-component="composer-voice-input" data-bf-part="time" aria-hidden="true">
            {formatElapsedTime(elapsedSeconds)}
          </span>

          <span
            className={`bitfun-chat-input__voice-pill-timeline${recording ? '' : ' bitfun-chat-input__voice-pill-timeline--paused'}`}
            data-bf-component="composer-voice-input"
            data-bf-part="timeline"
            aria-hidden="true"
          >
            {visibleTimelineSamples.map((sample, index) => {
              const scale = Math.max(0.12, Math.min(1, 0.12 + sample * 0.88));
              return (
                <span
                  key={index}
                  className="bitfun-chat-input__voice-pill-timeline-bar"
                  data-bf-component="composer-voice-input"
                  data-bf-part="timelineBar"
                  style={{
                    opacity: sample === 0 ? 0.32 : 0.82,
                    transform: `scaleY(${scale})`,
                  }}
                />
              );
            })}
          </span>

          <span className="bitfun-chat-input__voice-pill-divider" data-bf-component="composer-voice-input" data-bf-part="divider" aria-hidden="true" />

          <span data-bf-component="composer-voice-input" data-bf-part="action" data-bf-action="cancel" data-bf-state={transcribing ? 'disabled' : undefined}>
            <Tooltip content={controller.cancelTooltip}>
              <IconButton
                aria-label={controller.cancelTooltip}
                className="bitfun-chat-input__voice-pill-action bitfun-chat-input__voice-pill-action--cancel"
                size="sm"
                disabled={transcribing}
                onClick={(event) => {
                  event.stopPropagation();
                  controller.cancel();
                }}
                icon={<X size={16} />}
              />
            </Tooltip>
          </span>

          <span data-bf-component="composer-voice-input" data-bf-part="action" data-bf-action="transcribe" data-bf-state={controlsDisabled ? 'disabled' : undefined}>
            <Tooltip content={controlsDisabled ? controller.tooltip : controller.transcribeTooltip}>
              <IconButton
                aria-label={controlsDisabled ? controller.tooltip : controller.transcribeTooltip}
                className="bitfun-chat-input__voice-pill-action bitfun-chat-input__voice-pill-action--transcribe"
                size="sm"
                disabled={controlsDisabled}
                onClick={(event) => {
                  event.stopPropagation();
                  controller.transcribe();
                }}
                icon={transcribing && controller.completionMode === 'transcribe' ? (
                  <Loader2 size={15} className="bitfun-chat-input__voice-spinner" />
                ) : (
                  <Check size={16} />
                )}
              />
            </Tooltip>
          </span>

          <span data-bf-component="composer-voice-input" data-bf-part="action" data-bf-action="send" data-bf-state={controlsDisabled ? 'disabled' : undefined}>
            <Tooltip content={controlsDisabled ? controller.tooltip : controller.sendTooltip}>
              <IconButton
                aria-label={controlsDisabled ? controller.tooltip : controller.sendTooltip}
                className="bitfun-chat-input__voice-pill-send"
                tone="danger"
                size="sm"
                disabled={controlsDisabled}
                onClick={(event) => {
                  event.stopPropagation();
                  controller.transcribeAndSend();
                }}
                icon={transcribing && controller.completionMode === 'send' ? (
                  <Loader2 size={15} className="bitfun-chat-input__voice-spinner" />
                ) : (
                  <ArrowUp size={15} strokeWidth={2.5} />
                )}
              />
            </Tooltip>
          </span>
        </span>
      </span>
    );
  }

  return (
    <span className="bitfun-chat-input__voice-cluster" data-bf-component="composer-voice-input" data-bf-part="root" data-bf-phase="idle">
      <span className="bitfun-chat-input__voice-control-shell" data-bf-component="composer-voice-input" data-bf-part="control" data-bf-state={controller.disabled ? 'disabled' : undefined}>
        <Tooltip content={controller.tooltip}>
          <IconButton
            aria-label={controller.tooltip}
            className="bitfun-chat-input__voice-control"
            size="sm"
            disabled={controller.disabled}
            onClick={(event) => {
              event.stopPropagation();
              controller.toggle();
            }}
            icon={<Mic size={14} />}
          />
        </Tooltip>
      </span>
    </span>
  );
}
