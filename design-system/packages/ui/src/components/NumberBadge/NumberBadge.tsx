import { forwardRef, type HTMLAttributes, type ReactNode } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./NumberBadge.module.css";

export interface NumberBadgeProps extends Omit<HTMLAttributes<HTMLSpanElement>, "children"> {
  /** The caller owns formatting and any accessible contextual label. */
  value: ReactNode;
}

export const NumberBadge = forwardRef<HTMLSpanElement, NumberBadgeProps>(function NumberBadge({
  className,
  value,
  ...props
}, ref) {
  return (
    <span
      {...props}
      className={classNames(styles.root, className)}
      data-openbitfun-component="number-badge"
      ref={ref}
    >
      <span className={styles.value} data-openbitfun-part="value">{value}</span>
    </span>
  );
});
