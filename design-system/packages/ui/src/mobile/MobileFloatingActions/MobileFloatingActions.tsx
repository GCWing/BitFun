import {
  forwardRef,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileFloatingActions.module.css";

export interface MobileFloatingActionsProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  leading?: ReactNode;
  trailing?: ReactNode;
}

export const MobileFloatingActions = forwardRef<HTMLDivElement, MobileFloatingActionsProps>(
  function MobileFloatingActions({ className, leading, trailing, ...props }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.root, className)}
        data-bf-component="mobile-floating-actions"
        ref={ref}
      >
        <div className={styles.leading} data-bf-part="leading">{leading}</div>
        <div className={styles.trailing} data-bf-part="trailing">{trailing}</div>
      </div>
    );
  },
);
