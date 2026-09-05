import { useCallback, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Loader2, Phone } from 'lucide-react';
import { Icon } from '@openbitfun/ui';

import { RealtimeVoiceCallPanel } from './RealtimeVoiceCallPanel';
import { useRealtimeVoiceCall } from './RealtimeVoiceCallContext';
import type { VoiceMiniAppCallTarget } from './voiceClientContext';
import './ConversationModeSurface.scss';

interface ConversationModeSurfaceProps {
  children: ReactNode;
  className?: string;
  /** Captured MiniApp route; absence keeps the existing workspace voice flow. */
  voiceTarget?: VoiceMiniAppCallTarget;
  /** Prevents a MiniApp call from falling back to the workspace route while its session binds. */
  voiceStartDisabled?: boolean;
  switchTestId?: string;
}

/**
 * Shared text/realtime-voice capability shell for compact conversation hosts.
 * Window-specific components own only their chrome and provide the text
 * surface; this component is the single owner of mode switching and voice UI.
 */
export function ConversationModeSurface({
  children,
  className,
  voiceTarget,
  voiceStartDisabled = false,
  switchTestId,
}: ConversationModeSurfaceProps) {
  const { t } = useTranslation('settings/voice-input');
  const {
    enabled,
    phase,
    start: startVoiceCall,
    end: endVoiceCall,
  } = useRealtimeVoiceCall();
  const isVoiceMode = phase !== 'idle';
  const isTransitioning = phase === 'connecting' || phase === 'ending';
  const showModeSwitch = enabled || isVoiceMode;

  const handleModeSwitch = useCallback(() => {
    if (isVoiceMode) {
      endVoiceCall();
      return;
    }
    if (!enabled || voiceStartDisabled) return;
    startVoiceCall(voiceTarget);
  }, [enabled, endVoiceCall, isVoiceMode, startVoiceCall, voiceStartDisabled, voiceTarget]);

  return (
    <div
      className={[
        'openbitfun-conversation-mode-surface',
        className,
      ].filter(Boolean).join(' ')}
      data-openbitfun-component="conversation-mode-surface"
      data-openbitfun-part="root"
      data-openbitfun-state={isVoiceMode ? 'voice' : 'chat'}
    >
      <div
        className="openbitfun-conversation-mode-surface__body"
        data-openbitfun-component="conversation-mode-surface"
        data-openbitfun-part="body"
      >
        {isVoiceMode ? <RealtimeVoiceCallPanel /> : children}
      </div>

      {showModeSwitch ? (
        <footer
          className="openbitfun-conversation-mode-surface__switch"
          data-openbitfun-component="conversation-mode-surface"
          data-openbitfun-part="modeSwitch"
        >
          <button
            type="button"
            className={`openbitfun-conversation-mode-surface__switch-button${isVoiceMode ? ' is-voice' : ''}`}
            data-testid={switchTestId}
            data-openbitfun-component="conversation-mode-surface"
            data-openbitfun-part="modeSwitchButton"
            aria-pressed={isVoiceMode}
            disabled={phase === 'ending' || (!isVoiceMode && voiceStartDisabled)}
            onClick={handleModeSwitch}
          >
            {isTransitioning ? (
              <Loader2
                className="openbitfun-conversation-mode-surface__switch-spinner"
                size={15}
                aria-hidden="true"
              />
            ) : isVoiceMode ? (
              <Icon name="side-chat" size="sm" aria-hidden="true" />
            ) : (
              <Phone size={15} aria-hidden="true" />
            )}
            <span>
              {t(isVoiceMode
                ? 'voiceCall.call.switchToChat'
                : 'voiceCall.call.switchToVoice')}
            </span>
          </button>
        </footer>
      ) : null}
    </div>
  );
}

/** Shared voice identity used by compact-host headers while Voice is active. */
export function ConversationVoiceModeIcon() {
  return (
    <span
      className="openbitfun-conversation-voice-mode-icon"
      data-openbitfun-component="conversation-mode-surface"
      data-openbitfun-part="voiceModeIcon"
      aria-hidden="true"
    >
      <Phone size={14} />
    </span>
  );
}
