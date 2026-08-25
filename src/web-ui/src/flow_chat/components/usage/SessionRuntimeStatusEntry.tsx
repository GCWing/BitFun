import React from 'react';
import { Button } from '@bitfun/ui';
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
      <Button data-bf-component="session-runtime-status-entry" data-bf-part="root"
        className="session-runtime-status-entry"
        leadingIcon={<Activity size={13} />}
        onClick={onOpen}
        aria-label={t('usage.runtime.open')}
        size="sm"
        variant="fill"
      >
        <span data-bf-component="session-runtime-status-entry" data-bf-part="label">{t('usage.runtime.button')}</span>
      </Button>
    </Tooltip>
  );
}

SessionRuntimeStatusEntry.displayName = 'SessionRuntimeStatusEntry';
