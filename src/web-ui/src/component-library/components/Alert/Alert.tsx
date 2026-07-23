/**
 * Alert component
 */

import React, { useState, forwardRef, ReactNode } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import './Alert.scss';

export interface AlertProps {
  /** Alert type */
  type?: 'success' | 'error' | 'warning' | 'info';
  /** Title */
  title?: ReactNode;
  /** Message */
  message: ReactNode;
  /** Description text */
  description?: string;
  /** Closable */
  closable?: boolean;
  /** Close callback */
  onClose?: () => void;
  /** Show icon */
  showIcon?: boolean;
  /** Custom class name */
  className?: string;
  /** Custom styles */
  style?: React.CSSProperties;
}

export const Alert = forwardRef<HTMLDivElement, AlertProps>(({
  type = 'info',
  title,
  message,
  description,
  closable = false,
  onClose,
  showIcon = true,
  className = '',
  style,
}, ref) => {
  const { t } = useI18n('components');
  const [visible, setVisible] = useState(true);

  const handleClose = () => {
    setVisible(false);
    onClose?.();
  };

  if (!visible) return null;

  // Minimal icons
  const icons = {
    success: (
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2">
        <path d="M13.5 4L6 11.5L2.5 8" strokeLinecap="round" strokeLinejoin="round"/>
      </svg>
    ),
    error: (
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2">
        <circle cx="8" cy="8" r="6" />
        <path d="M8 4.5V8.5M8 11H8.01" strokeLinecap="round"/>
      </svg>
    ),
    warning: (
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2">
        <path d="M8 1.5L14.5 13H1.5L8 1.5Z" strokeLinejoin="round"/>
        <path d="M8 6V9M8 11H8.01" strokeLinecap="round"/>
      </svg>
    ),
    info: (
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2">
        <circle cx="8" cy="8" r="6" />
        <path d="M8 11V8M8 5H8.01" strokeLinecap="round"/>
      </svg>
    ),
  };

  const classNames = [
    'alert',
    `alert--${type}`,
    className
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div 
      ref={ref}
      className={classNames}
      style={style}
      role="alert"
      aria-live={type === 'error' ? 'assertive' : 'polite'}
    >
      {showIcon && (
        <div className="alert__icon">
          {icons[type]}
        </div>
      )}
      
      <div className="alert__content">
        {title && <div className="alert__title">{title}</div>}
        <div className="alert__message">{message}</div>
        {description && <div className="alert__description">{description}</div>}
      </div>

      {closable && (
        <button 
          className="alert__close"
          onClick={handleClose}
          aria-label={t('tooltip.close')}
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5">
            <path d="M1 1L11 11M11 1L1 11" strokeLinecap="round"/>
          </svg>
        </button>
      )}
    </div>
  );
});

Alert.displayName = 'Alert';
