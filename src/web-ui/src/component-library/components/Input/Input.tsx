/**
 * Input component
 */

import React, { forwardRef, useId } from 'react';
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
  const generatedId = useId();
  const inputId = props.id ?? `${generatedId}-input`;
  const supportId = `${generatedId}-support`;
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
  const appearanceState = [error && 'error', disabled && 'disabled'].filter(Boolean).join(' ');

  return (
    <div
      className={classNames}
      data-bf-component="input"
      data-bf-part="root"
      data-bf-variant={variant}
      data-bf-size={resolvedInputSize}
      data-bf-state={appearanceState || undefined}
    >
      {label && <label className="bitfun-input-label" htmlFor={inputId} data-bf-component="input" data-bf-part="label">{label}</label>}
      <div className="bitfun-input-container" data-bf-component="input" data-bf-part="container">
        {prefix && <span className="bitfun-input-prefix" data-bf-component="input" data-bf-part="prefix">{prefix}</span>}
        <input
          {...props}
          ref={ref}
          id={inputId}
          className="bitfun-input"
          data-bf-component="input"
          data-bf-part="control"
          disabled={disabled}
          aria-invalid={error || undefined}
          aria-describedby={(error && errorMessage) || (!error && hint) ? supportId : props['aria-describedby']}
        />
        {suffix && <span className="bitfun-input-suffix" data-bf-component="input" data-bf-part="suffix">{suffix}</span>}
      </div>
      {!error && hint && (
        <span id={supportId} className="bitfun-input-hint" data-bf-component="input" data-bf-part="message">{hint}</span>
      )}
      {error && errorMessage && (
        <span id={supportId} className="bitfun-input-error-message" role="alert" data-bf-component="input" data-bf-part="message">{errorMessage}</span>
      )}
    </div>
  );
});

Input.displayName = 'Input';
