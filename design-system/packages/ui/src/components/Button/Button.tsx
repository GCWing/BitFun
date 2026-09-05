import {
  forwardRef,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Button.module.css";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: ReactNode;
  leadingIcon?: ReactNode;
  loading?: boolean;
  size?: "xs" | "sm" | "md" | "lg";
  tone?: "danger" | "neutral";
  trailingIcon?: ReactNode;
  variant?: "fill" | "outline" | "primary" | "secondary" | "text";
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
      data-openbitfun-component="button"
      data-openbitfun-part="root"
      data-openbitfun-tone={tone}
      data-openbitfun-variant={variant}
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
