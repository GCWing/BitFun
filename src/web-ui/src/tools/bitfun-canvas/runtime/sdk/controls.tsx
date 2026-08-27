import { Checkbox as BitFunCheckbox } from '@/component-library/components/Checkbox/Checkbox';
import { Input as BitFunInput } from '@/component-library/components/Input/Input';
import { Tooltip as BitFunTooltip } from '@/component-library/components/Tooltip/Tooltip';
import { IconButton as BitFunIconButton, Switch as BitFunSwitch } from '@bitfun/ui';
import { Textarea as BitFunTextarea } from '@/component-library/components/Textarea/Textarea';
import type {
  CanvasCheckboxProps,
  CanvasIconButtonProps,
  CanvasSelectOption,
  CanvasSelectProps,
  CanvasTextAreaProps,
  CanvasTextInputProps,
  CanvasToggleProps,
} from './types';

function controlSize(size: 'sm' | 'small' | 'md' | 'medium' | 'lg' | 'large' | undefined) {
  if (size === 'sm') return 'small';
  if (size === 'md') return 'medium';
  if (size === 'lg') return 'large';
  return size ?? 'medium';
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
): Pick<React.ComponentProps<typeof BitFunIconButton>, 'tone' | 'variant'> {
  if (variant === 'danger') return { tone: 'danger', variant: 'quiet' };
  if (variant === 'primary' || variant === 'ai') return { variant: 'primary' };
  if (variant === 'success' || variant === 'warning') return { variant: 'fill' };
  return { variant: 'quiet' };
}

function selectSizeClass(size: CanvasSelectProps['size']) {
  return `bf-select--${controlSize(size)}`;
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
    <BitFunSwitch
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
    <label className="bf-canvas-toggle">
      {control}
      <span className="bf-canvas-toggle__copy">
        {label ? <span className="bf-canvas-toggle__label">{label}</span> : null}
        {description ? (
          <span className="bf-canvas-toggle__description">{description}</span>
        ) : null}
        {statusText ? <span className="bf-canvas-toggle__status">{statusText}</span> : null}
      </span>
    </label>
  );
}

export function Checkbox({ onChange, size, ...props }: CanvasCheckboxProps) {
  return (
    <BitFunCheckbox
      {...props}
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
  size,
  ...props
}: CanvasSelectProps) {
  const normalizedOptions = options.map(normalizeOption);
  const selectClassName = ['bf-select', selectSizeClass(size), className].filter(Boolean).join(' ');

  return (
    <select
      {...props}
      className={selectClassName}
      onChange={event => onChange?.(event.target.value)}
    >
      {placeholder ? <option value="">{placeholder}</option> : null}
      {normalizedOptions.map(option => (
        <option
          key={option.value}
          value={option.value}
          disabled={option.disabled}
        >
          {option.label ?? option.value}
        </option>
      ))}
    </select>
  );
}

export function TextInput({ onChange, size, ...props }: CanvasTextInputProps) {
  return (
    <BitFunInput
      {...props}
      size={controlSize(size)}
      onChange={event => onChange?.(event.target.value)}
    />
  );
}

export function TextArea({ onChange, ...props }: CanvasTextAreaProps) {
  return (
    <BitFunTextarea
      {...props}
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
    <BitFunIconButton
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
    <BitFunTooltip content={tooltipContent}>{control}</BitFunTooltip>
  ) : control;
}
