import {
  forwardRef,
  type ChangeEventHandler,
  type ReactNode,
  type SelectHTMLAttributes,
} from "react";
import { classNames } from "../../internal/classNames";
import { useFieldSurface } from "../../internal/fieldSurface";
import { Icon } from "../Icon";
import styles from "./Select.module.css";

export type SelectValue = string | number;
export type SelectSize = "sm" | "md" | "lg";

export interface SelectOption {
  disabled?: boolean;
  group?: string;
  label: string;
  testAttributes?: Record<`data-${string}`, string | number | boolean | undefined>;
  testId?: string;
  value: SelectValue;
}

export interface SelectProps
  extends Omit<
    SelectHTMLAttributes<HTMLSelectElement>,
    "children" | "defaultValue" | "onChange" | "size" | "value"
  > {
  defaultValue?: SelectValue;
  invalid?: boolean;
  leading?: ReactNode;
  onValueChange?: (value: SelectValue) => void;
  options: readonly SelectOption[];
  placeholder?: string;
  size?: SelectSize;
  value?: SelectValue;
}

function toNativeValue(value: SelectValue | undefined) {
  return value === undefined ? undefined : String(value);
}

function groupOptions(options: readonly SelectOption[]) {
  const groups = new Map<string, SelectOption[]>();
  const ungrouped: SelectOption[] = [];

  for (const option of options) {
    if (!option.group) {
      ungrouped.push(option);
      continue;
    }
    const group = groups.get(option.group) ?? [];
    group.push(option);
    groups.set(option.group, group);
  }

  return { groups, ungrouped };
}

function renderOption(option: SelectOption) {
  return (
    <option
      data-testid={option.testId}
      disabled={option.disabled}
      key={`${typeof option.value}:${option.value}`}
      value={String(option.value)}
      {...option.testAttributes}
    >
      {option.label}
    </option>
  );
}

export const Select = forwardRef<HTMLSelectElement, SelectProps>(function Select({
  className,
  defaultValue,
  disabled = false,
  invalid = false,
  leading,
  onValueChange,
  options,
  placeholder,
  size = "md",
  value,
  ...props
}, ref) {
  const { groups, ungrouped } = groupOptions(options);
  const fieldSurface = useFieldSurface();

  const handleChange: ChangeEventHandler<HTMLSelectElement> = (event) => {
    const option = options.find((candidate) => String(candidate.value) === event.currentTarget.value);
    if (option) onValueChange?.(option.value);
  };

  return (
    <span
      className={classNames(styles.root, className)}
      data-openbitfun-component="select"
      data-disabled={disabled ? "true" : "false"}
      data-field-surface={fieldSurface}
      data-has-leading={leading !== undefined && leading !== null ? "true" : "false"}
      data-invalid={invalid ? "true" : "false"}
      data-size={size}
    >
      {leading !== undefined && leading !== null && (
        <span aria-hidden="true" className={styles.leading} data-openbitfun-part="leading">
          {leading}
        </span>
      )}
      <select
        {...props}
        aria-invalid={invalid || undefined}
        className={styles.select}
        defaultValue={toNativeValue(defaultValue)}
        disabled={disabled}
        onChange={handleChange}
        ref={ref}
        value={toNativeValue(value)}
      >
        {placeholder !== undefined && (
          <option disabled value="">{placeholder}</option>
        )}
        {ungrouped.map(renderOption)}
        {[...groups].map(([label, group]) => (
          <optgroup key={label} label={label}>{group.map(renderOption)}</optgroup>
        ))}
      </select>
      <span aria-hidden="true" className={styles.indicator} data-openbitfun-part="indicator">
        <Icon name="chevron-down" size="sm" />
      </span>
    </span>
  );
});
