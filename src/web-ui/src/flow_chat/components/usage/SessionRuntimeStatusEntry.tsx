import React from 'react';
import { useTranslation } from 'react-i18next';
import { Activity } from 'lucide-react';
import { Tooltip } from '@openbitfun/ui';
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
      <button data-openbitfun-component="session-runtime-status-entry" data-openbitfun-part="root"
        className="session-runtime-status-entry"
        type="button"
        onClick={onOpen}
        aria-label={t('usage.runtime.open')}
      >
        <Activity size={13} data-openbitfun-component="session-runtime-status-entry" data-openbitfun-part="icon" aria-hidden />
        <span data-openbitfun-component="session-runtime-status-entry" data-openbitfun-part="label">{t('usage.runtime.button')}</span>
      </button>
    </Tooltip>
  );
}

SessionRuntimeStatusEntry.displayName = 'SessionRuntimeStatusEntry';
