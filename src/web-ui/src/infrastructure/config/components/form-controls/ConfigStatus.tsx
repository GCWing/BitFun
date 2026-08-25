import React from 'react';
import { X } from 'lucide-react';
import { IconButton } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';

export interface ConfigStatusProps {
   
  type: 'success' | 'error' | 'warning' | 'info';
   
  message: string;
   
  icon?: React.ReactNode;
   
  closable?: boolean;
   
  onClose?: () => void;
   
  style?: React.CSSProperties;
   
  className?: string;
   
  multiline?: boolean;
}

const defaultIcons = {
  success: '✅',
  error: '❌',
  warning: '⚠️',
  info: 'ℹ️'
};

export const ConfigStatus: React.FC<ConfigStatusProps> = ({
  type,
  message,
  icon,
  closable = false,
  onClose,
  style,
  className = '',
  multiline = false
}) => {
  const { t } = useI18n('common');
  const statusClass = `config-form-status ${type} ${className}`.trim();
  const displayIcon = icon !== undefined ? icon : defaultIcons[type];
  
  return (
    <div className={statusClass} style={style}>
      {displayIcon && <span>{displayIcon}</span>}
      <div 
        style={{ 
          flex: 1,
          whiteSpace: multiline ? 'pre-line' : 'nowrap',
          overflow: multiline ? 'visible' : 'hidden',
          textOverflow: multiline ? 'unset' : 'ellipsis'
        }}
      >
        {message}
      </div>
      {closable && onClose && (
        <IconButton
          type="button"
          size="xs"
          onClick={onClose}
          style={{ marginLeft: '8px' }}
          aria-label={t('actions.close')}
        >
          <X size={14} />
        </IconButton>
      )}
    </div>
  );
};

ConfigStatus.displayName = 'ConfigStatus';
