import {
  forwardRef,
  type ChangeEventHandler,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileFileButton.module.css";

export interface MobileFileButtonProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "children" | "type"> {
  children: ReactNode;
  leading?: ReactNode;
  loading?: boolean;
  onChange?: ChangeEventHandler<HTMLInputElement>;
}

export const MobileFileButton = forwardRef<HTMLInputElement, MobileFileButtonProps>(
  function MobileFileButton({
    children,
    className,
    disabled,
    leading,
    loading = false,
    ...props
  }, ref) {
    return (
      <label
        aria-disabled={disabled || loading || undefined}
        className={classNames(styles.root, className)}
        data-openbitfun-component="mobile-file-button"
        data-loading={loading ? "true" : "false"}
      >
        {loading
          ? <span aria-hidden="true" className={styles.spinner} />
          : leading && <span className={styles.slot}>{leading}</span>}
        <span>{children}</span>
        <input
          {...props}
          className={styles.input}
          disabled={disabled || loading}
          ref={ref}
          type="file"
        />
      </label>
    );
  },
);
