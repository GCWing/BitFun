import {
  forwardRef,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Toolbar.module.css";
import { NumberBadge } from "../NumberBadge";

export type ToolbarSize = "sm" | "md";
export type ToolbarLeadingOverflow = "visible" | "scroll";
export type ToolbarGroupGap = "sm" | "md";

export interface ToolbarProps extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  bordered?: boolean;
  center?: ReactNode;
  leading?: ReactNode;
  leadingOverflow?: ToolbarLeadingOverflow;
  size?: ToolbarSize;
  trailing?: ReactNode;
}

export interface ToolbarGroupProps extends HTMLAttributes<HTMLDivElement> {
  gap?: ToolbarGroupGap;
}

export type ToolbarBadgeProps = HTMLAttributes<HTMLSpanElement>;
export type ToolbarSeparatorProps = HTMLAttributes<HTMLSpanElement>;

export const Toolbar = forwardRef<HTMLDivElement, ToolbarProps>(function Toolbar({
  bordered = true,
  center,
  className,
  leading,
  leadingOverflow = "visible",
  size = "sm",
  trailing,
  ...props
}, ref) {
  const hasCenter = center !== undefined && center !== null;

  return (
    <div
      {...props}
      className={classNames(styles.root, className)}
      data-bf-component="toolbar"
      data-bordered={bordered ? "true" : "false"}
      data-has-center={hasCenter ? "true" : "false"}
      data-size={size}
      ref={ref}
    >
      {leading !== undefined && leading !== null && (
        <div
          className={styles.leading}
          data-bf-part="leading"
          data-overflow={leadingOverflow}
        >
          {leading}
        </div>
      )}
      {hasCenter && (
        <div className={styles.center} data-bf-part="center">{center}</div>
      )}
      {trailing !== undefined && trailing !== null && (
        <div className={styles.trailing} data-bf-part="trailing">{trailing}</div>
      )}
    </div>
  );
});

export const ToolbarGroup = forwardRef<HTMLDivElement, ToolbarGroupProps>(
  function ToolbarGroup({ children, className, gap = "sm", ...props }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.group, className)}
        data-bf-part="group"
        data-gap={gap}
        ref={ref}
      >
        {children}
      </div>
    );
  },
);

export const ToolbarBadge = forwardRef<HTMLSpanElement, ToolbarBadgeProps>(
  function ToolbarBadge({ children, className, ...props }, ref) {
    return (
      <NumberBadge {...props} className={classNames(styles.badge, className)} data-bf-part="badge" ref={ref} value={children} />
    );
  },
);

export const ToolbarSeparator = forwardRef<HTMLSpanElement, ToolbarSeparatorProps>(
  function ToolbarSeparator({ className, ...props }, ref) {
    return (
      <span
        {...props}
        aria-hidden="true"
        className={classNames(styles.separator, className)}
        data-bf-part="separator"
        ref={ref}
      />
    );
  },
);
