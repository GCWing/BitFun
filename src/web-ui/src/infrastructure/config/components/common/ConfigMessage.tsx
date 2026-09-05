import React from 'react';
import { Alert } from '@openbitfun/ui';
import './ConfigPageState.scss';

export interface ConfigMessageData {
  type: 'success' | 'error' | 'info' | 'warning';
  text: string;
}

export interface ConfigMessageProps {
  message: ConfigMessageData | null;
  className?: string;
}

export const ConfigMessage: React.FC<ConfigMessageProps> = ({
  message,
  className = '',
}) => {
  if (!message) return null;

  return (
    <div
      className={['openbitfun-config-message', className].filter(Boolean).join(' ')}
      data-openbitfun-component="config"
      data-openbitfun-part="message"
    >
      <Alert tone={message.type} message={message.text} />
    </div>
  );
};
