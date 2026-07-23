/**
 * Input component
 */

import React, { forwardRef } from 'react';
import './Input.scss';

export interface InputProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size' | 'prefix'> {
  variant?: 'default' | 'filled' | 'outlined';
  inputSize?: 'small' | 'medium' | 'large';
  size?: 'small' | 'medium' | 'large';
  error?: boolean;
  errorMessage?: string;
  prefix?: React.ReactNode;
  suffix?: React.ReactNode;
  label?: string;
  hint?: React.ReactNode;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(({
  variant = 'default',
  inputSize = 'medium',
  size,
  error = false,
  errorMessage,
  prefix,
  suffix,
  label,
  hint,
  className = '',
  disabled,
  ...props
}, ref) => {
  const resolvedInputSize = size ?? inputSize;
  const classNames = [
    'bitfun-input-wrapper',
    `bitfun-input-wrapper--${variant}`,
    `bitfun-input-wrapper--${resolvedInputSize}`,
    error && 'bitfun-input-wrapper--error',
    disabled && 'bitfun-input-wrapper--disabled',
    className
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className={classNames}>
      {label && <label className="bitfun-input-label">{label}</label>}
      <div className="bitfun-input-container">
        {prefix && <span className="bitfun-input-prefix">{prefix}</span>}
        <input
          ref={ref}
          className="bitfun-input"
          disabled={disabled}
          {...props}
        />
        {suffix && <span className="bitfun-input-suffix">{suffix}</span>}
      </div>
      {!error && hint && (
        <span className="bitfun-input-error-message">{hint}</span>
      )}
      {error && errorMessage && (
        <span className="bitfun-input-error-message">{errorMessage}</span>
      )}
    </div>
  );
});

Input.displayName = 'Input';
