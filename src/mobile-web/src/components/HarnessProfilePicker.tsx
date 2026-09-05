import React from 'react';
import { MobileChoiceSheet } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';

interface HarnessProfilePickerProps {
  open: boolean;
  onClose: () => void;
  onSelect: (agentType: string) => void;
}

const PROFILES = [
  { agentType: 'minimal', labelKey: 'sessions.harnessMinimal', density: 1 },
  { agentType: 'agentic', labelKey: 'sessions.harnessStandard', density: 2 },
  { agentType: 'Ultra', labelKey: 'sessions.harnessUltimate', density: 3 },
] as const;

/** Creation-time Harness selector shared by the home and workspace entry points. */
const HarnessProfilePicker: React.FC<HarnessProfilePickerProps> = ({
  open,
  onClose,
  onSelect,
}) => {
  const { t } = useI18n();

  return (
    <MobileChoiceSheet
      cancelLabel={t('sessions.cancel')}
      className="harness-profile-picker"
      onOpenChange={() => onClose()}
      onSelect={onSelect}
      open={open}
      options={PROFILES.map((profile) => ({
        label: t(profile.labelKey),
        leading: (
          <span className="harness-profile-picker__density" aria-hidden="true">
            {Array.from({ length: profile.density }, (_, index) => (
              <span key={index} style={{ height: `${8 + index * 5}px` }} />
            ))}
          </span>
        ),
        value: profile.agentType,
      }))}
      title={t('sessions.selectExecutionMode')}
    />
  );
};

export default HarnessProfilePicker;
