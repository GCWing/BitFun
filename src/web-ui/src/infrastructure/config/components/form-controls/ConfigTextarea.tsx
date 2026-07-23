import React, { forwardRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Textarea } from '@/component-library';

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
  ...props
}, ref) => {
  const { t } = useTranslation('common');
  
  const textareaStyle = {
    minHeight: `${minHeight}px`,
    ...(maxHeight && { maxHeight: `${maxHeight}px` }),
    ...style
  };

  
  
  const textareaElement = (
    <Textarea
      ref={ref}
      label={labelIcon ? undefined : label} 
      error={!!error}
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
        <label className={`config-form-label ${required ? 'required' : ''}`}>
          {labelIcon}
          {label}
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
