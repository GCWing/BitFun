import {
  forwardRef,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./StatusPill.module.css";

export type StatusPillTone =
  | "neutral"
  | "accent"
  | "info"
  | "success"
  | "warning"
  | "danger";

export interface StatusPillProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, "children"> {
  children: ReactNode;
  leading?: ReactNode;
  tone?: StatusPillTone;
}

export const StatusPill = forwardRef<HTMLSpanElement, StatusPillProps>(
  function StatusPill({
    children,
    className,
    leading,
    tone = "success",
    ...props
  }, ref) {
    return (
      <span
        {...props}
        className={classNames(styles.root, className)}
        data-openbitfun-component="status-pill"
        data-tone={tone}
        ref={ref}
      >
        {leading !== undefined && leading !== null && (
          <span aria-hidden="true" className={styles.leading} data-openbitfun-part="leading">
            {leading}
          </span>
        )}
        <span className={styles.label} data-openbitfun-part="label">{children}</span>
      </span>
    );
  },
);
