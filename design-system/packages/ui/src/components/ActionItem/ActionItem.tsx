import {
  forwardRef,
  type ButtonHTMLAttributes,
  type MouseEventHandler,
  type ReactNode,
} from "react";
import { IconButton, type IconButtonProps } from "../IconButton";
import { classNames } from "../../internal/classNames";
import styles from "./ActionItem.module.css";

export interface ActionItemAction {
  disabled?: boolean;
  icon: ReactNode;
  id: string;
  label: string;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  tone?: IconButtonProps["tone"];
}

export interface ActionItemProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children" | "className"> {
  actions?: readonly ActionItemAction[];
  children: ReactNode;
  className?: string;
  leading?: ReactNode;
  reserveLeadingSpace?: boolean;
  shortcut?: ReactNode;
  triggerClassName?: string;
}

export const ActionItem = forwardRef<HTMLButtonElement, ActionItemProps>(function ActionItem({
  actions = [],
  children,
  className,
  disabled,
  leading,
  reserveLeadingSpace = false,
  shortcut,
  triggerClassName,
  type = "button",
  ...props
}, ref) {
  const hasLeadingArea = reserveLeadingSpace || leading !== undefined && leading !== null;

  return (
    <span
      className={classNames(styles.root, className)}
      data-bf-component="action-item"
      data-disabled={disabled ? "true" : "false"}
    >
      <button
        {...props}
        className={classNames(styles.trigger, triggerClassName)}
        data-bf-part="trigger"
        disabled={disabled}
        ref={ref}
        type={type}
      >
        {hasLeadingArea && (
          <span aria-hidden="true" className={styles.leading} data-bf-part="leading">
            {leading}
          </span>
        )}
        <span className={styles.label} data-bf-part="label">{children}</span>
        {shortcut !== undefined && shortcut !== null && (
          <span aria-hidden="true" className={styles.shortcut} data-bf-part="shortcut">
            {shortcut}
          </span>
        )}
      </button>
      {actions.length > 0 && (
        <span className={styles.actions} data-bf-part="actions">
          {actions.map((action) => (
            <IconButton
              aria-label={action.label}
              disabled={disabled || action.disabled}
              icon={action.icon}
              key={action.id}
              onClick={action.onClick}
              size="sm"
              tone={action.tone}
              variant="quiet"
            />
          ))}
        </span>
      )}
    </span>
  );
});
