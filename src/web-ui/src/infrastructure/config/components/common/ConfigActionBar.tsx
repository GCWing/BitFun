import React from 'react';
import { Button } from '@openbitfun/ui';
import { useTranslation } from 'react-i18next';
import {
  ConfigFieldStatus,
  type ConfigFieldStatusState,
} from './ConfigFieldStatus';

export interface ConfigActionBarProps extends React.HTMLAttributes<HTMLDivElement> {
  status: ConfigFieldStatusState;
  statusMessage?: React.ReactNode;
  saving?: boolean;
  saveDisabled?: boolean;
  discardDisabled?: boolean;
  saveLabel?: React.ReactNode;
  discardLabel?: React.ReactNode;
  onSave: () => void;
  onDiscard: () => void;
}

export const ConfigActionBar: React.FC<ConfigActionBarProps> = ({
  status,
  statusMessage,
  saving = false,
  saveDisabled = false,
  discardDisabled = false,
  saveLabel,
  discardLabel,
  onSave,
  onDiscard,
  className = '',
  ...props
}) => {
  const { t } = useTranslation('settings');

  if (status === 'saved' && !statusMessage) {
    return null;
  }

  return (
    <div
      {...props}
      className={['openbitfun-config-action-bar', className].filter(Boolean).join(' ')}
      data-openbitfun-component="config"
      data-openbitfun-part="actionBar"
    >
      <ConfigFieldStatus status={status} message={statusMessage} />
      {status !== 'saved' ? (
        <div className="openbitfun-config-action-bar__actions">
          <Button
            size="sm"
            variant="outline"
            disabled={saving || discardDisabled}
            onClick={onDiscard}
          >
            {discardLabel ?? t('changeActions.discard')}
          </Button>
          <Button
            size="sm"
            variant="primary"
            loading={saving}
            disabled={saving || saveDisabled}
            onClick={onSave}
          >
            {saveLabel ?? t('changeActions.save')}
          </Button>
        </div>
      ) : null}
    </div>
  );
};
