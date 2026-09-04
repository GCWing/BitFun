import {
  forwardRef,
  useEffect,
  useRef,
  useImperativeHandle,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Checkbox.module.css";

export type CheckboxSize = "sm" | "md" | "lg";

export interface CheckboxProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "children" | "size" | "type"> {
  children?: ReactNode;
  description?: ReactNode;
  indeterminate?: boolean;
  invalid?: boolean;
  label?: ReactNode;
  onCheckedChange?: (checked: boolean) => void;
  size?: CheckboxSize;
}

export const Checkbox = forwardRef<HTMLInputElement, CheckboxProps>(function Checkbox({
  checked,
  children,
  className,
  defaultChecked,
  description,
  disabled = false,
  indeterminate = false,
  invalid = false,
  label,
  onChange,
  onCheckedChange,
  size = "md",
  ...props
}, forwardedRef) {
  const inputRef = useRef<HTMLInputElement>(null);
  useImperativeHandle(forwardedRef, () => inputRef.current as HTMLInputElement);

  useEffect(() => {
    if (inputRef.current) inputRef.current.indeterminate = indeterminate;
  }, [indeterminate]);

  const hasContent = label !== undefined || description !== undefined || children !== undefined;

  return (
    <label
      className={classNames(styles.root, className)}
      data-openbitfun-component="checkbox"
      data-disabled={disabled ? "true" : "false"}
      data-indeterminate={indeterminate ? "true" : "false"}
      data-invalid={invalid ? "true" : "false"}
      data-size={size}
    >
      <span className={styles.control} data-openbitfun-part="control">
        <input
          {...props}
          aria-invalid={invalid || undefined}
          checked={checked}
          className={styles.input}
          defaultChecked={defaultChecked}
          disabled={disabled}
          onChange={(event) => {
            onChange?.(event);
            if (!event.defaultPrevented) onCheckedChange?.(event.currentTarget.checked);
          }}
          ref={inputRef}
          type="checkbox"
        />
        <span aria-hidden="true" className={styles.box} data-openbitfun-part="box">
          <svg className={styles.icon} viewBox="0 0 16 16">
            {indeterminate
              ? <path d="M4 8h8" />
              : <path d="m3.5 8 3 3 6-6" />}
          </svg>
        </span>
      </span>
      {hasContent && (
        <span className={styles.content} data-openbitfun-part="content">
          {label !== undefined && <span className={styles.label}>{label}</span>}
          {description !== undefined && <span className={styles.description}>{description}</span>}
          {children}
        </span>
      )}
    </label>
  );
});
