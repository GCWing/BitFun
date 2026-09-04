import {
  forwardRef,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./LauncherButton.module.css";

export interface LauncherButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: ReactNode;
  leadingIcon?: ReactNode;
}

export const LauncherButton = forwardRef<
  HTMLButtonElement,
  LauncherButtonProps
>(function LauncherButton({
  children,
  className,
  leadingIcon,
  type = "button",
  ...props
}, ref) {
  return (
    <button
      {...props}
      className={classNames(styles.root, className)}
      data-openbitfun-component="launcher-button"
      data-openbitfun-part="root"
      ref={ref}
      type={type}
    >
      {leadingIcon !== undefined && leadingIcon !== null && (
        <span aria-hidden="true" className={styles.icon} data-openbitfun-part="icon">
          {leadingIcon}
        </span>
      )}
      <span className={styles.label} data-openbitfun-part="label">
        {children}
      </span>
    </button>
  );
});
