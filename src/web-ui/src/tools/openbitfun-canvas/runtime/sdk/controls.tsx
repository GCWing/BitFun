import {
  Checkbox as OpenBitFunCheckbox,
  Field as DesignField,
  IconButton as OpenBitFunIconButton,
  Input as DesignInput,
  Select as OpenBitFunSelect,
  Switch as OpenBitFunSwitch,
  Textarea as OpenBitFunTextarea,
  Tooltip as OpenBitFunTooltip,
} from '@openbitfun/ui';
import type {
  CanvasCheckboxProps,
  CanvasIconButtonProps,
  CanvasSelectOption,
  CanvasSelectProps,
  CanvasTextAreaProps,
  CanvasTextInputProps,
  CanvasToggleProps,
} from './types';
import { canvasArrayProp } from './runtimeValidation';

function controlSize(size: 'sm' | 'small' | 'md' | 'medium' | 'lg' | 'large' | undefined) {
  if (size === 'sm' || size === 'small') return 'sm';
  if (size === 'lg' || size === 'large') return 'lg';
  return 'md';
}

function designSystemControlSize(
  size: CanvasIconButtonProps['size'],
): 'sm' | 'md' | 'lg' {
  if (size === 'sm' || size === 'small') return 'sm';
  if (size === 'lg' || size === 'large') return 'lg';
  return 'md';
}

function designSystemIconButtonStyle(
  variant: CanvasIconButtonProps['variant'],
): Pick<React.ComponentProps<typeof OpenBitFunIconButton>, 'tone' | 'variant'> {
  if (variant === 'danger') return { tone: 'danger', variant: 'quiet' };
  if (variant === 'primary' || variant === 'ai') return { variant: 'primary' };
  if (variant === 'success' || variant === 'warning') return { variant: 'fill' };
  return { variant: 'quiet' };
}

function normalizeOption(option: string | number | CanvasSelectOption): CanvasSelectOption {
  if (typeof option === 'string' || typeof option === 'number') {
    return { label: option, value: option };
  }
  return option;
}

export function Toggle({
  onChange,
  size: _size,
  label,
  description,
  loading = false,
  checkedText,
  uncheckedText,
  disabled,
  checked,
  ...props
}: CanvasToggleProps) {
  const statusText = checked ? checkedText : uncheckedText;
  const control = (
    <OpenBitFunSwitch
      {...props}
      checked={checked}
      disabled={disabled || loading}
      aria-busy={loading || props['aria-busy']}
      aria-label={props['aria-label'] ?? label}
      onChange={event => onChange?.(event.target.checked)}
    />
  );

  if (!label && !description && !statusText) {
    return control;
  }

  return (
    <label className="openbitfun-canvas-toggle">
      {control}
      <span className="openbitfun-canvas-toggle__copy">
        {label ? <span className="openbitfun-canvas-toggle__label">{label}</span> : null}
        {description ? (
          <span className="openbitfun-canvas-toggle__description">{description}</span>
        ) : null}
        {statusText ? <span className="openbitfun-canvas-toggle__status">{statusText}</span> : null}
      </span>
    </label>
  );
}

export function Checkbox({ error, onChange, size, ...props }: CanvasCheckboxProps) {
  return (
    <OpenBitFunCheckbox
      {...props}
      invalid={error}
      size={controlSize(size)}
      onChange={event => onChange?.(event.target.checked)}
    />
  );
}

export function Select({
  options = [],
  placeholder,
  onChange,
  className,
  children: _children,
  defaultValue,
  size,
  ...props
}: CanvasSelectProps) {
  const normalizedOptions = canvasArrayProp<string | number | CanvasSelectOption>(
    'Select',
    'options',
    options,
  ).map(normalizeOption);
  const selectClassName = ['openbitfun-select', className].filter(Boolean).join(' ');
  const normalizedDefaultValue = Array.isArray(defaultValue) ? defaultValue[0] : defaultValue;

  return (
    <OpenBitFunSelect
      {...props}
      className={selectClassName}
      defaultValue={normalizedDefaultValue}
      onValueChange={value => onChange?.(String(value))}
      options={normalizedOptions.map(option => ({
        disabled: option.disabled,
        label: typeof option.label === 'string' ? option.label : String(option.value),
        value: option.value,
      }))}
      placeholder={placeholder === undefined ? undefined : String(placeholder)}
      size={designSystemControlSize(size)}
    />
  );
}

export function TextInput({
  onChange,
  size,
  label,
  hint,
  prefix,
  suffix,
  error,
  errorMessage,
  ...props
}: CanvasTextInputProps) {
  const control = (
    <DesignInput
      {...props}
      invalid={error}
      leading={prefix}
      trailing={suffix}
      size={designSystemControlSize(size)}
      onChange={event => onChange?.(event.target.value)}
    />
  );
  const fieldError = error ? errorMessage : undefined;
  if (label === undefined && hint === undefined && fieldError === undefined) {
    return control;
  }
  return (
    <DesignField label={label} description={hint} error={fieldError} controlWidth="fill">
      {control}
    </DesignField>
  );
}

export function TextArea({ error, onChange, ...props }: CanvasTextAreaProps) {
  return (
    <OpenBitFunTextarea
      {...props}
      invalid={error}
      onChange={event => onChange?.(event.target.value)}
    />
  );
}

export function IconButton({
  'aria-label': ariaLabel,
  children,
  isLoading = false,
  size,
  title,
  tooltip,
  variant,
  ...props
}: CanvasIconButtonProps) {
  const resolvedLabel = ariaLabel
    ?? (typeof tooltip === 'string' ? tooltip : undefined)
    ?? (typeof title === 'string' ? title : 'Action');
  const control = (
    <OpenBitFunIconButton
      {...props}
      {...designSystemIconButtonStyle(variant)}
      aria-label={resolvedLabel}
      icon={children}
      loading={isLoading}
      size={designSystemControlSize(size)}
      title={typeof title === 'string' ? title : undefined}
    />
  );

  const tooltipContent = tooltip ?? title;
  return tooltipContent ? (
    <OpenBitFunTooltip content={tooltipContent}>{control}</OpenBitFunTooltip>
  ) : control;
}
