import React, { forwardRef, useRef, useImperativeHandle, useCallback, useId } from 'react';
import './Textarea.scss';

export interface TextareaProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: string;
  error?: boolean;
  errorMessage?: string;
  hint?: string;
  autoResize?: boolean;
  showCount?: boolean;
  maxLength?: number;
  variant?: 'default' | 'filled' | 'outlined';
  className?: string;
  'data-bf-component'?: string;
  'data-bf-part'?: string;
}

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(
  (
    {
      label,
      error = false,
      errorMessage,
      hint,
      autoResize = false,
      showCount = false,
      maxLength,
      variant = 'default',
      className = '',
      value,
      onChange,
      style,
      'data-bf-component': rootAppearanceComponent = 'textarea',
      'data-bf-part': rootAppearancePart = 'root',
      ...props
    },
    ref
  ) => {
    const generatedId = useId();
    const textareaId = props.id ?? `${generatedId}-textarea`;
    const supportId = `${generatedId}-support`;
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const [charCount, setCharCount] = React.useState(0);

    useImperativeHandle(ref, () => textareaRef.current!);

    const adjustHeight = useCallback(() => {
      if (autoResize && textareaRef.current) {
        textareaRef.current.style.height = 'auto';
        textareaRef.current.style.height = `${textareaRef.current.scrollHeight}px`;
      }
    }, [autoResize]);

    React.useEffect(() => {
      adjustHeight();
    }, [value, adjustHeight]);

    React.useEffect(() => {
      const count = typeof value === 'string' ? value.length : 0;
      setCharCount(count);
    }, [value]);

    const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const newValue = e.target.value;
      setCharCount(newValue.length);
      adjustHeight();
      onChange?.(e);
    };

    const containerClass = [
      'bitfun-textarea',
      `bitfun-textarea--${variant}`,
      error && 'bitfun-textarea--error',
      props.disabled && 'bitfun-textarea--disabled',
      className
    ].filter(Boolean).join(' ');
    const rootAppearanceProps: Record<string, string> = {};
    rootAppearanceProps['data-bf-component'] = rootAppearanceComponent;
    rootAppearanceProps['data-bf-part'] = rootAppearancePart;

    return (
      <div className={containerClass} data-bf-component="textarea" data-bf-part="root" data-bf-variant={variant} data-bf-state={[error && 'error', props.disabled && 'disabled', autoResize && 'autoResize'].filter(Boolean).join(' ') || undefined} {...rootAppearanceProps}>
        {label && (
          <label className="bitfun-textarea__label" data-bf-component="textarea" data-bf-part="label">
            {label}
            {props.required && <span className="bitfun-textarea__required" data-bf-component="textarea" data-bf-part="required">*</span>}
          </label>
        )}
        <div className="bitfun-textarea__wrapper" data-bf-component="textarea" data-bf-part="wrapper">
          <textarea
            {...props}
            ref={textareaRef}
            id={textareaId}
            className="bitfun-textarea__field"
            value={value}
            onChange={handleChange}
            maxLength={maxLength}
            aria-invalid={error || undefined}
            aria-describedby={(error && errorMessage) || (!error && hint) || showCount
              ? supportId
              : props['aria-describedby']}
            style={style}
            {...props}
            data-bf-component="textarea"
            data-bf-part="field"
          />
        </div>
        {(hint || errorMessage || showCount) && (
          <div className="bitfun-textarea__footer" data-bf-component="textarea" data-bf-part="footer">
            <div className="bitfun-textarea__hint-wrapper">
              {error && errorMessage && (
                <span className="bitfun-textarea__error-message" data-bf-component="textarea" data-bf-part="message">{errorMessage}</span>
              )}
              {!error && hint && (
                <span className="bitfun-textarea__hint" data-bf-component="textarea" data-bf-part="message">{hint}</span>
              )}
            </div>
            {showCount && (
              <span className="bitfun-textarea__count" data-bf-component="textarea" data-bf-part="count">
                {charCount}{maxLength && ` / ${maxLength}`}
              </span>
            )}
          </div>
        )}
      </div>
    );
  }
);

Textarea.displayName = 'Textarea';
