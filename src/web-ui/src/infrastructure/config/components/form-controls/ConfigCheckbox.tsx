import React, { forwardRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Checkbox } from '@/component-library';

export interface ConfigCheckboxProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size'> {
   
  label?: string;
   
  description?: string;
   
  hint?: string;
   
  error?: string;
   
  success?: boolean;
   
  labelIcon?: React.ReactNode;
   
  inline?: boolean;
   
  compact?: boolean;
}

export const ConfigCheckbox = forwardRef<HTMLInputElement, ConfigCheckboxProps>(({
  label,
  description,
  hint,
  error,
  success,
  labelIcon,
  inline = false,
  className = '',
  style,
  children,
  ...props
}, ref) => {
  const { t } = useTranslation('common');
  
  const labelText = labelIcon ? (
    <>
      {labelIcon}
      {label}
    </>
  ) : label;

  
  const checkboxElement = (
    <Checkbox
      ref={ref}
      label={labelText}
      description={description}
      error={!!error}
      className={className}
      {...props}
    >
      {children}
    </Checkbox>
  );

  if (inline) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px', ...style }}>
        {checkboxElement}
        {hint && <span className="config-form-hint">{hint}</span>}
      </div>
    );
  }

  return (
    <div className="config-form-group" style={style}>
      {checkboxElement}
      {hint && !error && !success && <span className="config-form-hint">{hint}</span>}
      {error && (
        <div className="config-form-status error">
          <span>{error}</span>
        </div>
      )}
      {success && (
        <div className="config-form-status success">
          <span>{t('form.validationSuccess.checkbox')}</span>
        </div>
      )}
    </div>
  );
});

ConfigCheckbox.displayName = 'ConfigCheckbox';
