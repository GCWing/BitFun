import React from 'react';
import { Mic } from 'lucide-react';
import { IconButton } from '@/component-library';
import { useTranslation } from 'react-i18next';
import { useVoiceInput } from '@/infrastructure/speech/useVoiceInput';
import './VoiceInputButton.scss';

export interface VoiceInputButtonProps {
  onTranscript: (text: string) => void;
  className?: string;
}

export const VoiceInputButton: React.FC<VoiceInputButtonProps> = ({
  onTranscript,
  className,
}) => {
  const { t } = useTranslation('flow-chat');
  const voice = useVoiceInput({ onTranscript });

  if (!voice.enabled) {
    return null;
  }

  if (voice.phase === 'recording' || voice.phase === 'transcribing') {
    return (
      <div className={`voice-input-button ${voice.phase}${className ? ` ${className}` : ''}`}>
        <span className="voice-input-button__dot" />
        <span className="voice-input-button__label">
          {voice.phase === 'recording'
            ? t('input.voiceInput.recording')
            : t('input.voiceInput.transcribing')}
        </span>
        {voice.phase === 'recording' && (
          <>
            <button
              type="button"
              className="voice-input-button__cancel"
              onClick={voice.cancel}
              title={t('input.voiceInput.cancel')}
            >
              {t('input.voiceInput.cancel')}
            </button>
            <button
              type="button"
              className="voice-input-button__done"
              onClick={voice.transcribe}
              title={t('input.voiceInput.done')}
            >
              {t('input.voiceInput.done')}
            </button>
          </>
        )}
      </div>
    );
  }

  return (
    <IconButton
      size="small"
      variant="ghost"
      disabled={voice.disabled}
      tooltip={voice.tooltip}
      onClick={voice.toggle}
      className={`voice-input-button${className ? ` ${className}` : ''}`}
    >
      <Mic size={14} />
    </IconButton>
  );
};
