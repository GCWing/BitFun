import {
  forwardRef,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Input.module.css";

export type InputVariant = "default" | "search";

export interface InputProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  containerClassName?: string;
  leadingIcon?: ReactNode;
  trailingContent?: ReactNode;
  variant?: InputVariant;
  "data-bf-preview-state"?: "focus" | "hover";
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input({
  className,
  containerClassName,
  disabled,
  leadingIcon,
  trailingContent,
  type = "text",
  variant = "default",
  "data-bf-preview-state": previewState,
  ...props
}, ref) {
  return (
    <span
      className={classNames(styles.root, containerClassName)}
      data-bf-component="input"
      data-bf-part="root"
      data-bf-preview-state={previewState}
      data-disabled={disabled || undefined}
      data-variant={variant}
    >
      {leadingIcon && (
        <span aria-hidden="true" className={styles.icon} data-bf-part="leadingIcon">
          {leadingIcon}
        </span>
      )}
      <input
        {...props}
        className={classNames(styles.input, className)}
        disabled={disabled}
        ref={ref}
        type={type}
      />
      {trailingContent && (
        <span className={styles.trailing} data-bf-part="trailingContent">
          {trailingContent}
        </span>
      )}
    </span>
  );
});
