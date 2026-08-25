import {
  forwardRef,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Button.module.css";

export type ButtonTone = "danger" | "neutral" | "primary";
export type ButtonVariant = "outline" | "fill" | "text";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: ReactNode;
  leadingIcon?: ReactNode;
  loading?: boolean;
  size?: "sm" | "md" | "lg";
  tone?: ButtonTone;
  trailingIcon?: ReactNode;
  variant?: ButtonVariant;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button({
  children,
  className,
  disabled,
  leadingIcon,
  loading = false,
  size = "md",
  tone = "neutral",
  trailingIcon,
  type = "button",
  variant = "outline",
  ...props
}, ref) {
  return (
    <button
      {...props}
      aria-busy={loading || undefined}
      className={classNames(styles.button, className)}
      data-bf-component="button"
      data-bf-part="root"
      data-bf-tone={tone}
      data-bf-variant={variant}
      data-loading={loading ? "true" : "false"}
      data-size={size}
      disabled={disabled || loading}
      ref={ref}
      type={type}
    >
      <span aria-hidden="true" className={styles.progress} />
      <span className={styles.content}>
        {leadingIcon && (
          <span aria-hidden="true" className={classNames(styles.icon, styles.leadingIcon)}>
            {leadingIcon}
          </span>
        )}
        <span className={styles.label}>{children}</span>
        {trailingIcon && (
          <span aria-hidden="true" className={classNames(styles.icon, styles.trailingIcon)}>
            {trailingIcon}
          </span>
        )}
      </span>
    </button>
  );
});
