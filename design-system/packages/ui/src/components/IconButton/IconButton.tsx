import {
  forwardRef,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./IconButton.module.css";

export type IconButtonSize = "sm" | "md" | "lg";
export type IconButtonTone = "danger" | "neutral" | "primary";

type AccessibleIconButtonName =
  | { "aria-label": string; "aria-labelledby"?: string }
  | { "aria-label"?: string; "aria-labelledby": string };

type IconButtonBaseProps = Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "aria-label" | "aria-labelledby" | "children"
> & {
  children: ReactNode;
  loading?: boolean;
  size?: IconButtonSize;
  tone?: IconButtonTone;
};

export type IconButtonProps = IconButtonBaseProps & AccessibleIconButtonName;

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  function IconButton({
    children,
    className,
    disabled,
    loading = false,
    size = "sm",
    tone = "neutral",
    type = "button",
    ...props
  }, ref) {
    return (
      <button
        {...props}
        aria-busy={loading || undefined}
        className={classNames(styles.iconButton, className)}
        data-bf-component="icon-button"
        data-bf-part="root"
        data-bf-tone={tone}
        data-loading={loading ? "true" : "false"}
        data-size={size}
        disabled={disabled || loading}
        ref={ref}
        type={type}
      >
        <span aria-hidden="true" className={styles.progress} />
        <span aria-hidden="true" className={styles.icon}>
          {children}
        </span>
      </button>
    );
  },
);
