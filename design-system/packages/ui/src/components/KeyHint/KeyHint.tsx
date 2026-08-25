import {
  forwardRef,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./KeyHint.module.css";

export interface KeyHintProps extends HTMLAttributes<HTMLElement> {
  children: ReactNode;
  leadingIcon?: ReactNode;
}

export const KeyHint = forwardRef<HTMLElement, KeyHintProps>(function KeyHint({
  children,
  className,
  leadingIcon,
  ...props
}, ref) {
  return (
    <kbd
      {...props}
      className={classNames(styles.keyHint, className)}
      data-bf-component="key-hint"
      data-bf-part="root"
      ref={ref}
    >
      {leadingIcon && (
        <span aria-hidden="true" className={styles.icon} data-bf-part="icon">
          {leadingIcon}
        </span>
      )}
      <span data-bf-part="label">{children}</span>
    </kbd>
  );
});
