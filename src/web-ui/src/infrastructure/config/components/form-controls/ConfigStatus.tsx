import React from 'react';

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
        <button
          onClick={onClose}
          style={{
            background: 'none',
            border: 'none',
            color: 'inherit',
            cursor: 'pointer',
            padding: '0',
            marginLeft: '8px',
            fontSize: '16px',
            lineHeight: '1',
            opacity: 0.7,
            transition: 'opacity 0.2s ease'
          }}
          onMouseEnter={(e) => { e.currentTarget.style.opacity = '1'; }}
          onMouseLeave={(e) => { e.currentTarget.style.opacity = '0.7'; }}
        >
          ×
        </button>
      )}
    </div>
  );
};

ConfigStatus.displayName = 'ConfigStatus';
