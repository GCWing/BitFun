import React, { forwardRef, useId } from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@openbitfun/ui';

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
  id,
  ...props
}, ref) => {
  const { t } = useTranslation('common');
  const generatedId = useId();
  const controlId = id ?? `${generatedId}-config-input`;
  
  const inputElement = (
    <Input
      ref={ref}
      id={controlId}
      required={required || undefined}
      trailing={rightIcon}
      invalid={!!error}
      className={className}
      {...props}
    />
  );

  if (inline) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px', ...style }}>
        {label && (
          <label className="config-form-label" htmlFor={controlId}>
            {labelIcon}
            <span className="config-form-label__text">
              {label}
              {required ? (
                <span
                  aria-hidden="true"
                  className="config-form-label__required"
                  data-openbitfun-component="config"
                  data-openbitfun-part="required"
                >
                  *
                </span>
              ) : null}
            </span>
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
        <label className="config-form-label" htmlFor={controlId}>
          {labelIcon}
          <span className="config-form-label__text">
            {label}
            {required ? (
              <span
                aria-hidden="true"
                className="config-form-label__required"
                data-openbitfun-component="config"
                data-openbitfun-part="required"
              >
                *
              </span>
            ) : null}
          </span>
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
