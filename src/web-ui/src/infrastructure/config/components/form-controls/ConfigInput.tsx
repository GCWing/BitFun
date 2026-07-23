import React, { forwardRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/component-library';

export interface ConfigInputProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size'> {
   
  label?: string;
   
  required?: boolean;
   
  hint?: string;
   
  error?: string;
   
  success?: boolean;
   
  labelIcon?: React.ReactNode;
   
  rightIcon?: React.ReactNode;
   
  inline?: boolean;
}

export const ConfigInput = forwardRef<HTMLInputElement, ConfigInputProps>(({
  label,
  required = false,
  hint,
  error,
  success,
  labelIcon,
  rightIcon,
  inline = false,
  className = '',
  style,
  ...props
}, ref) => {
  const { t } = useTranslation('common');
  
  const inputElement = (
    <Input
      ref={ref}
      suffix={rightIcon}
      error={!!error}
      errorMessage={error}
      className={className}
      {...props}
    />
  );

  if (inline) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px', ...style }}>
        {label && (
          <label className={`config-form-label ${required ? 'required' : ''}`}>
            {labelIcon}
            {label}
          </label>
        )}
        <div style={{ flex: 1 }}>
          {inputElement}
        </div>
        {hint && <span className="config-form-hint">{hint}</span>}
      </div>
    );
  }

  return (
    <div className="config-form-group" style={style}>
      {label && (
        <label className={`config-form-label ${required ? 'required' : ''}`}>
          {labelIcon}
          {label}
        </label>
      )}
      {inputElement}
      {hint && !error && !success && <span className="config-form-hint">{hint}</span>}
      {error && (
        <div className="config-form-status error">
          <span>{error}</span>
        </div>
      )}
      {success && (
        <div className="config-form-status success">
          <span>{t('form.validationSuccess.input')}</span>
        </div>
      )}
    </div>
  );
});

ConfigInput.displayName = 'ConfigInput';
