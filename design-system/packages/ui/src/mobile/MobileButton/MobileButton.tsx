import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileButton.module.css";

export type MobileButtonAppearance = "primary" | "secondary" | "plain" | "danger";
export type MobileButtonSize = "sm" | "md" | "lg";

export interface MobileButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  appearance?: MobileButtonAppearance;
  block?: boolean;
  leading?: ReactNode;
  loading?: boolean;
  size?: MobileButtonSize;
  trailing?: ReactNode;
}

export const MobileButton = forwardRef<HTMLButtonElement, MobileButtonProps>(
  function MobileButton({
    appearance = "secondary",
    block = false,
    children,
    className,
    disabled,
    leading,
    loading = false,
    size = "md",
    trailing,
    type = "button",
    ...props
  }, ref) {
    return (
      <button
        {...props}
        aria-busy={loading || undefined}
        className={classNames(styles.root, className)}
        data-appearance={appearance}
        data-bf-component="mobile-button"
        data-block={block ? "true" : "false"}
        data-loading={loading ? "true" : "false"}
        data-size={size}
        disabled={disabled || loading}
        ref={ref}
        type={type}
      >
        {loading && <span aria-hidden="true" className={styles.spinner} data-bf-part="progress" />}
        {!loading && leading !== undefined && leading !== null && (
          <span className={styles.slot} data-bf-part="leading">{leading}</span>
        )}
        <span className={styles.label} data-bf-part="label">{children}</span>
        {!loading && trailing !== undefined && trailing !== null && (
          <span className={styles.slot} data-bf-part="trailing">{trailing}</span>
        )}
      </button>
    );
  },
);
