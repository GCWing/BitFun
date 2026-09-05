import { forwardRef, type ReactNode, type TextareaHTMLAttributes } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileTextarea.module.css";
export interface MobileTextareaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  invalid?: boolean;
  leading?: ReactNode;
  trailing?: ReactNode;
}

export const MobileTextarea = forwardRef<HTMLTextAreaElement, MobileTextareaProps>(
  function MobileTextarea({
    "aria-invalid": ariaInvalid,
    className,
    disabled,
    invalid = false,
    leading,
    trailing,
    ...props
  }, ref) {
    const resolvedInvalid = invalid || ariaInvalid === true || ariaInvalid === "true";
    return (
      <div
        className={styles.root}
        data-openbitfun-component="mobile-textarea"
        data-disabled={disabled ? "true" : "false"}
        data-invalid={resolvedInvalid ? "true" : "false"}
      >
        {leading && <span className={styles.slot}>{leading}</span>}
        <textarea
          {...props}
          aria-invalid={resolvedInvalid || undefined}
          className={classNames(styles.input, className)}
          disabled={disabled}
          ref={ref}
        />
        {trailing && <span className={styles.slot}>{trailing}</span>}
      </div>
    );
  },
);
