import React from 'react';
import { useTranslation } from 'react-i18next';
import './ConfigPageState.scss';

export type ConfigFieldStatusState = 'unsaved' | 'saving' | 'saved' | 'error';

export interface ConfigFieldStatusProps extends React.HTMLAttributes<HTMLDivElement> {
  status: ConfigFieldStatusState;
  message?: React.ReactNode;
}

export const ConfigFieldStatus: React.FC<ConfigFieldStatusProps> = ({
  status,
  message,
  className = '',
  ...props
}) => {
  const { t } = useTranslation('settings');
  return (
    <div
      {...props}
      className={['openbitfun-config-field-status', className].filter(Boolean).join(' ')}
      data-openbitfun-component="config"
      data-openbitfun-part="fieldStatus"
      data-openbitfun-status={status}
      role={status === 'error' ? 'alert' : 'status'}
      aria-live={status === 'error' ? 'assertive' : 'polite'}
    >
      <span className="openbitfun-config-field-status__marker" aria-hidden="true" />
      <span>{message ?? t(`changeStatus.${status}`)}</span>
    </div>
  );
};
