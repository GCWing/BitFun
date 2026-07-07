import { Checkbox as BitFunCheckbox } from '@/component-library/components/Checkbox/Checkbox';
import { IconButton as BitFunIconButton } from '@/component-library/components/IconButton/IconButton';
import { Input as BitFunInput } from '@/component-library/components/Input/Input';
import { Switch as BitFunSwitch } from '@/component-library/components/Switch/Switch';
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

function selectSizeClass(size: CanvasSelectProps['size']) {
  return `bf-select--${controlSize(size)}`;
}

function normalizeOption(option: string | number | CanvasSelectOption): CanvasSelectOption {
  if (typeof option === 'string' || typeof option === 'number') {
    return { label: option, value: option };
  }
  return option;
}

export function Toggle({ onChange, size, ...props }: CanvasToggleProps) {
  return (
    <BitFunSwitch
      {...props}
      size={controlSize(size)}
      onChange={event => onChange?.(event.target.checked)}
    />
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

export function IconButton({ size, title, tooltip, ...props }: CanvasIconButtonProps) {
  return (
    <BitFunIconButton
      {...props}
      size={controlSize(size)}
      tooltip={tooltip ?? title}
      title={typeof title === 'string' ? title : undefined}
    />
  );
}
