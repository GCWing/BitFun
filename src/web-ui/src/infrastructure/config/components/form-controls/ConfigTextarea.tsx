import React, { forwardRef, useId } from 'react';
import { useTranslation } from 'react-i18next';
import { Textarea } from '@openbitfun/ui';

export interface ConfigTextareaProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
   
  label?: string;
   
  required?: boolean;
   
  hint?: string;
   
  error?: string;
   
  success?: boolean;
   
  labelIcon?: React.ReactNode;
   
  minHeight?: number;
   
  maxHeight?: number;
   
  showCount?: boolean;
   
  autoResize?: boolean;
}

export const ConfigTextarea = forwardRef<HTMLTextAreaElement, ConfigTextareaProps>(({
  label,
  required = false,
  hint,
  error,
  success,
  labelIcon,
  minHeight = 80,
  maxHeight,
  showCount = false,
  autoResize = false,
  className = '',
  style,
  maxLength,
  value = '',
  onChange,
  id,
  ...props
}, ref) => {
  const { t } = useTranslation('common');
  const generatedId = useId();
  const controlId = id ?? `${generatedId}-config-textarea`;
  
  const textareaStyle = {
    minHeight: `${minHeight}px`,
    ...(maxHeight && { maxHeight: `${maxHeight}px` }),
    ...style
  };

  
  
  const textareaElement = (
    <Textarea
      ref={ref}
      id={controlId}
      label={labelIcon ? undefined : label} 
      required={required || undefined}
      invalid={Boolean(error)}
      errorMessage={error}
      hint={hint}
      showCount={showCount}
      maxLength={maxLength}
      autoResize={autoResize}
      className={className}
      style={textareaStyle}
      value={value}
      onChange={onChange}
      {...props}
    />
  );

  return (
    <div className="config-form-group">
      {label && labelIcon && (
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
      {textareaElement}
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

ConfigTextarea.displayName = 'ConfigTextarea';
