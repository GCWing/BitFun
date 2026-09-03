import React, { useEffect } from 'react';
import { createPortal } from 'react-dom';
import { useI18n } from '../i18n';

interface HarnessProfilePickerProps {
  open: boolean;
  onClose: () => void;
  onSelect: (agentType: string) => void;
}

const PROFILES = [
  { id: 'minimal', agentType: 'minimal', labelKey: 'sessions.harnessMinimal', density: 1 },
  { id: 'standard', agentType: 'agentic', labelKey: 'sessions.harnessStandard', density: 2 },
  { id: 'ultimate', agentType: 'Ultra', labelKey: 'sessions.harnessUltimate', density: 3 },
] as const;

/** Creation-time Harness selector shared by the home and workspace entry points. */
const HarnessProfilePicker: React.FC<HarnessProfilePickerProps> = ({
  open,
  onClose,
  onSelect,
}) => {
  const { t } = useI18n();

  useEffect(() => {
    if (!open) return undefined;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose, open]);

  if (!open) return null;

  return createPortal(
    <div className="session-list__menu-overlay" onClick={onClose}>
      <div
        className="session-list__menu-sheet harness-profile-picker"
        role="dialog"
        aria-modal="true"
        aria-labelledby="harness-profile-picker-title"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="session-list__menu-handle" />
        <div className="session-list__menu-title" id="harness-profile-picker-title">
          {t('sessions.selectExecutionMode')}
        </div>
        <div className="session-list__menu-actions">
          {PROFILES.map((profile) => (
            <button
              key={profile.id}
              type="button"
              className="session-list__menu-btn harness-profile-picker__option"
              onClick={() => onSelect(profile.agentType)}
            >
              <span className="harness-profile-picker__density" aria-hidden="true">
                {Array.from({ length: profile.density }, (_, index) => (
                  <span key={index} style={{ height: `${8 + index * 5}px` }} />
                ))}
              </span>
              <span>{t(profile.labelKey)}</span>
            </button>
          ))}
        </div>
        <button type="button" className="session-list__menu-cancel" onClick={onClose}>
          {t('sessions.cancel')}
        </button>
      </div>
    </div>,
    document.body,
  );
};

export default HarnessProfilePicker;
