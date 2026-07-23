import React from 'react';
import { useTranslation } from 'react-i18next';
import { Select, type SelectOption } from '@/component-library';

export interface ConfigSelectOption {
  value: string | number;
  label: string;
  disabled?: boolean;
  group?: string;
}

export interface ConfigSelectProps {
   
  label?: string;
   
  required?: boolean;
   
  hint?: string;
   
  error?: string;
   
  success?: boolean;
   
  labelIcon?: React.ReactNode;
   
  options?: ConfigSelectOption[];
   
  placeholder?: string;
   
  inline?: boolean;
   
  value?: string | number;
   
  defaultValue?: string | number;
   
  disabled?: boolean;
   
  onChange?: (value: string | number) => void;
   
  size?: 'small' | 'medium' | 'large';
   
  className?: string;
   
  style?: React.CSSProperties;
}

export const ConfigSelect: React.FC<ConfigSelectProps> = ({
  label,
  required = false,
  hint,
  error,
  success,
  labelIcon,
  options = [],
  placeholder,
  inline = false,
  value,
  defaultValue,
  disabled = false,
  onChange,
  size = 'medium',
  className = '',
  style,
}) => {
  const { t } = useTranslation('common');
  
  const selectOptions: SelectOption[] = options.map(opt => ({
    value: opt.value,
    label: opt.label,
    disabled: opt.disabled,
    group: opt.group,
  }));

  
  const handleChange = (newValue: string | number | (string | number)[]) => {
    if (onChange && !Array.isArray(newValue)) {
      onChange(newValue);
    }
  };

  
  const selectElement = (
    <Select
      options={selectOptions}
      value={value}
      defaultValue={defaultValue}
      placeholder={placeholder}
      disabled={disabled}
      onChange={handleChange}
      size={size}
      error={!!error}
      errorMessage={error}
      className={className}
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
        {selectElement}
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
      {selectElement}
      {hint && !error && !success && <span className="config-form-hint">{hint}</span>}
      {error && (
        <div className="config-form-status error">
          <span>{error}</span>
        </div>
      )}
      {success && (
        <div className="config-form-status success">
          <span>{t('form.validationSuccess.select')}</span>
        </div>
      )}
    </div>
  );
};

ConfigSelect.displayName = 'ConfigSelect';
