import {
  forwardRef,
  type ChangeEventHandler,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Input.module.css";

export interface InputProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "className" | "size"> {
  className?: string;
  inputClassName?: string;
  invalid?: boolean;
  leading?: ReactNode;
  onValueChange?: (value: string) => void;
  size?: "sm" | "md" | "lg";
  trailing?: ReactNode;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input({
  "aria-invalid": ariaInvalid,
  className,
  disabled,
  inputClassName,
  invalid = false,
  leading,
  onChange,
  onValueChange,
  size = "sm",
  trailing,
  type = "text",
  ...props
}, ref) {
  const handleChange: ChangeEventHandler<HTMLInputElement> = (event) => {
    onChange?.(event);
    onValueChange?.(event.currentTarget.value);
  };
  const resolvedAriaInvalid = invalid ? true : ariaInvalid;
  const isInvalid = resolvedAriaInvalid !== undefined
    && resolvedAriaInvalid !== false
    && resolvedAriaInvalid !== "false";

  return (
    <span
      className={classNames(styles.field, className)}
      data-bf-component="input"
      data-disabled={disabled ? "true" : "false"}
      data-invalid={isInvalid ? "true" : "false"}
      data-size={size}
    >
      {leading !== undefined && leading !== null && (
        <span className={styles.leading} data-bf-part="leading">{leading}</span>
      )}
      <input
        {...props}
        aria-invalid={resolvedAriaInvalid}
        className={classNames(styles.input, inputClassName)}
        disabled={disabled}
        onChange={handleChange}
        ref={ref}
        type={type}
      />
      {trailing !== undefined && trailing !== null && (
        <span className={styles.trailing} data-bf-part="trailing">{trailing}</span>
      )}
    </span>
  );
});
