import { forwardRef, type HTMLAttributes } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Spinner.module.css";

export type SpinnerSize = "xs" | "sm" | "md" | "lg";
export type SpinnerVariant = "bars" | "matrix";

export interface SpinnerProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, "children"> {
  size?: SpinnerSize;
  variant?: SpinnerVariant;
}

export const Spinner = forwardRef<HTMLSpanElement, SpinnerProps>(function Spinner({
  "aria-label": ariaLabel,
  className,
  size = "md",
  variant = "matrix",
  ...props
}, ref) {
  const cellCount = variant === "matrix" ? 9 : 3;
  return (
    <span
      {...props}
      aria-hidden={ariaLabel ? undefined : "true"}
      aria-label={ariaLabel}
      className={classNames(styles.root, className)}
      data-bf-component="spinner"
      data-size={size}
      data-variant={variant}
      ref={ref}
      role={ariaLabel ? "status" : undefined}
    >
      {Array.from({ length: cellCount }, (_, index) => (
        <span aria-hidden="true" className={styles.cell} key={index} />
      ))}
    </span>
  );
});

export interface LoadingStateProps
  extends HTMLAttributes<HTMLDivElement> {
  size?: SpinnerSize;
}

export const LoadingState = forwardRef<HTMLDivElement, LoadingStateProps>(
  function LoadingState({ children, className, size = "md", ...props }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.loadingState, className)}
        data-bf-component="loading-state"
        data-size={size}
        ref={ref}
      >
        <Spinner size={size} />
        {children ? <span className={styles.label} data-bf-part="label">{children}</span> : null}
      </div>
    );
  },
);
