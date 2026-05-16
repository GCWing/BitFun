import React from 'react';
import { useTranslation } from 'react-i18next';
import { Activity } from 'lucide-react';
import { Tooltip } from '@/component-library';
import './SessionRuntimeStatusEntry.scss';

interface SessionRuntimeStatusEntryProps {
  onOpen?: () => void;
}

export const SessionRuntimeStatusEntry: React.FC<SessionRuntimeStatusEntryProps> = ({
  onOpen,
}) => {
  if (!onOpen) {
    return null;
  }

  return <SessionRuntimeButton onOpen={onOpen} />;
};

function SessionRuntimeButton({
  onOpen,
}: {
  onOpen: () => void;
}) {
  const { t } = useTranslation('flow-chat');
  return (
    <Tooltip content={t('usage.runtime.tooltip')}>
      <button
        className="session-runtime-status-entry"
        type="button"
        onClick={onOpen}
        aria-label={t('usage.runtime.open')}
      >
        <Activity size={13} aria-hidden />
        <span>{t('usage.runtime.button')}</span>
      </button>
    </Tooltip>
  );
}

SessionRuntimeStatusEntry.displayName = 'SessionRuntimeStatusEntry';
