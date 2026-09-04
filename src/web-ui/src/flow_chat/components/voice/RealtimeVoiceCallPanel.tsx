import { Icon, IconButton, Tooltip } from '@openbitfun/ui';
import { Bot, Loader2, MicOff, PhoneOff } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useRealtimeVoiceCall } from './RealtimeVoiceCallContext';
import './RealtimeVoiceCall.scss';

/**
 * Realtime voice mode content for shared compact conversation surfaces.
 *
 * ConversationModeSurface owns the text/voice switch; this component only
 * renders the active call so compact hosts cannot drift into parallel voice
 * implementations.
 */
export function RealtimeVoiceCallPanel() {
  const { t } = useTranslation('settings/voice-input');
  const controller = useRealtimeVoiceCall();
  const connecting = controller.phase === 'connecting' || controller.phase === 'ending';

  return (
    <section
      className="openbitfun-realtime-call"
      data-openbitfun-component="realtime-voice-call"
      data-openbitfun-part="root"
      data-openbitfun-phase={controller.phase}
      role="region"
      aria-label={t('voiceCall.call.title')}
      aria-live="polite"
    >
      <header
        className="openbitfun-realtime-call__header"
        data-openbitfun-component="realtime-voice-call"
        data-openbitfun-part="header"
      >
        <span
          className="openbitfun-realtime-call__avatar"
          data-openbitfun-component="realtime-voice-call"
          data-openbitfun-part="avatar"
          aria-hidden="true"
        >
          {connecting ? <Loader2 size={18} className="openbitfun-realtime-call__spinner" /> : <Bot size={18} />}
        </span>
        <span
          className="openbitfun-realtime-call__heading"
          data-openbitfun-component="realtime-voice-call"
          data-openbitfun-part="heading"
        >
          <strong>{t('voiceCall.call.title')}</strong>
          <small>{controller.status}</small>
        </span>
        <span
          className="openbitfun-realtime-call__live-dot"
          data-openbitfun-component="realtime-voice-call"
          data-openbitfun-part="liveIndicator"
          aria-hidden="true"
        />
      </header>

      <div
        className="openbitfun-realtime-call__conversation"
        data-openbitfun-component="realtime-voice-call"
        data-openbitfun-part="conversation"
      >
        {controller.userTranscript ? (
          <div
            className="openbitfun-realtime-call__utterance openbitfun-realtime-call__utterance--user"
            data-openbitfun-component="realtime-voice-call"
            data-openbitfun-part="utterance"
          >
            <Icon name="user" size="lg" style={{ width: 14, height: 14 }} aria-hidden="true" />
            <span>{controller.userTranscript}</span>
          </div>
        ) : null}
        {controller.assistantTranscript ? (
          <div
            className="openbitfun-realtime-call__utterance openbitfun-realtime-call__utterance--assistant"
            data-openbitfun-component="realtime-voice-call"
            data-openbitfun-part="utterance"
          >
            <Bot size={14} aria-hidden="true" />
            <span>{controller.assistantTranscript}</span>
          </div>
        ) : null}
        {!controller.userTranscript && !controller.assistantTranscript ? (
          <div
            className="openbitfun-realtime-call__empty"
            data-openbitfun-component="realtime-voice-call"
            data-openbitfun-part="empty"
          >
            {t('voiceCall.call.empty')}
          </div>
        ) : null}
      </div>

      {controller.taskPhase ? (
        <div
          className="openbitfun-realtime-call__task"
          data-openbitfun-component="realtime-voice-call"
          data-openbitfun-part="task"
          data-openbitfun-state={controller.taskPhase.replace(/_/g, '-')}
        >
          <span className="openbitfun-realtime-call__task-pulse" aria-hidden="true" />
          <span>
            {controller.taskProgressText || t(`voiceCall.call.taskPhases.${controller.taskPhase}`)}
          </span>
        </div>
      ) : null}

      <div
        className="openbitfun-realtime-call__meter"
        data-openbitfun-component="realtime-voice-call"
        data-openbitfun-part="meter"
        aria-hidden="true"
      >
        {Array.from({ length: 18 }, (_, index) => {
          const distance = Math.abs(index - 8.5) / 8.5;
          const scale = controller.muted
            ? 0.08
            : Math.max(0.08, controller.audioLevel * (1 - distance * 0.72));
          return <span key={index} style={{ transform: `scaleY(${scale})` }} />;
        })}
      </div>

      <footer
        className="openbitfun-realtime-call__controls"
        data-openbitfun-component="realtime-voice-call"
        data-openbitfun-part="controls"
      >
        <Tooltip content={controller.muted ? t('voiceCall.call.unmute') : t('voiceCall.call.mute')}>
          <IconButton
            size="md"
            className="openbitfun-realtime-call__control"
            data-openbitfun-component="realtime-voice-call"
            data-openbitfun-part="control"
            aria-label={controller.muted ? t('voiceCall.call.unmute') : t('voiceCall.call.mute')}
            disabled={connecting}
            onClick={controller.toggleMute}
            icon={controller.muted
              ? <MicOff size={18} />
              : <Icon name="mic" size="lg" style={{ width: 18, height: 18 }} />}
          />
        </Tooltip>
        <Tooltip content={t('voiceCall.call.settings')}>
          <IconButton
            size="md"
            className="openbitfun-realtime-call__control"
            data-openbitfun-component="realtime-voice-call"
            data-openbitfun-part="control"
            aria-label={t('voiceCall.call.settings')}
            onClick={controller.openSettings}
            icon={<Icon name="settings" size="lg" style={{ width: 18, height: 18 }} />}
          />
        </Tooltip>
        <Tooltip content={t('voiceCall.call.hangUp')}>
          <IconButton
            size="md"
            className="openbitfun-realtime-call__control openbitfun-realtime-call__control--end"
            data-openbitfun-component="realtime-voice-call"
            data-openbitfun-part="control"
            aria-label={t('voiceCall.call.hangUp')}
            disabled={controller.phase === 'ending'}
            onClick={controller.end}
            icon={<PhoneOff size={19} />}
          />
        </Tooltip>
      </footer>
    </section>
  );
}
