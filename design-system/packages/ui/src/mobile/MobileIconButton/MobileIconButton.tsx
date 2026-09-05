import {
  forwardRef,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileIconButton.module.css";

export type MobileIconButtonAppearance = "plain" | "surface" | "floating";
export type MobileIconButtonSize = "sm" | "md";

export interface MobileIconButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label" | "children"> {
  "aria-label": string;
  appearance?: MobileIconButtonAppearance;
  icon: ReactNode;
  loading?: boolean;
  selected?: boolean;
  size?: MobileIconButtonSize;
}

export const MobileIconButton = forwardRef<HTMLButtonElement, MobileIconButtonProps>(
  function MobileIconButton({
    "aria-label": ariaLabel,
    appearance = "surface",
    className,
    disabled,
    icon,
    loading = false,
    selected,
    size = "md",
    type = "button",
    ...props
  }, ref) {
    return (
      <button
        {...props}
        aria-busy={loading || undefined}
        aria-label={ariaLabel}
        aria-pressed={selected}
        className={classNames(styles.root, className)}
        data-appearance={appearance}
        data-openbitfun-component="mobile-icon-button"
        data-loading={loading ? "true" : "false"}
        data-selected={selected === true ? "true" : "false"}
        data-size={size}
        disabled={disabled || loading}
        ref={ref}
        type={type}
      >
        <span aria-hidden="true" className={styles.spinner} data-openbitfun-part="progress" />
        <span aria-hidden="true" className={styles.icon} data-openbitfun-part="icon">
          {icon}
        </span>
      </button>
    );
  },
);
