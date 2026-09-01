import {
  forwardRef,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Radio.module.css";

export type RadioSize = "sm" | "md" | "lg";

export interface RadioProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "children" | "size" | "type"> {
  children?: ReactNode;
  description?: ReactNode;
  invalid?: boolean;
  label?: ReactNode;
  onCheckedChange?: (checked: boolean) => void;
  size?: RadioSize;
}

export const Radio = forwardRef<HTMLInputElement, RadioProps>(function Radio({
  checked,
  children,
  className,
  defaultChecked,
  description,
  disabled = false,
  invalid = false,
  label,
  onChange,
  onCheckedChange,
  size = "md",
  ...props
}, ref) {
  const hasContent = label !== undefined || description !== undefined || children !== undefined;

  return (
    <label
      className={classNames(styles.root, className)}
      data-bf-component="radio"
      data-disabled={disabled ? "true" : "false"}
      data-invalid={invalid ? "true" : "false"}
      data-size={size}
    >
      <span className={styles.control} data-bf-part="control">
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
          ref={ref}
          type="radio"
        />
        <span aria-hidden="true" className={styles.circle} data-bf-part="circle">
          <span className={styles.dot} />
        </span>
      </span>
      {hasContent && (
        <span className={styles.content} data-bf-part="content">
          {label !== undefined && <span className={styles.label}>{label}</span>}
          {description !== undefined && <span className={styles.description}>{description}</span>}
          {children}
        </span>
      )}
    </label>
  );
});
