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
  checked?: boolean;
  disabled?: boolean;
  icon: ReactNode;
  id: string;
  label: string;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  role?: "button" | "menuitem" | "menuitemcheckbox" | "menuitemradio";
  testAttributes?: Record<`data-${string}`, string | number | boolean | undefined>;
  testId?: string;
  tone?: IconButtonProps["tone"];
}

export type ActionItemTone = "neutral" | "danger";

export interface ActionItemProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children" | "className"> {
  actions?: readonly ActionItemAction[];
  children: ReactNode;
  className?: string;
  leading?: ReactNode;
  metadata?: ReactNode;
  reserveLeadingSpace?: boolean;
  shortcut?: ReactNode;
  tone?: ActionItemTone;
}

export const ActionItem = forwardRef<HTMLButtonElement, ActionItemProps>(function ActionItem({
  actions = [],
  children,
  className,
  disabled,
  leading,
  metadata,
  reserveLeadingSpace = false,
  shortcut,
  tone = "neutral",
  type = "button",
  ...props
}, ref) {
  const hasLeadingArea = reserveLeadingSpace || leading !== undefined && leading !== null;

  return (
    <span
      className={classNames(styles.root, className)}
      data-bf-component="action-item"
      data-bf-tone={tone}
      data-disabled={disabled ? "true" : "false"}
    >
      <button
        {...props}
        className={styles.trigger}
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
        {metadata !== undefined && metadata !== null && (
          <span className={styles.metadata} data-bf-part="metadata">
            {metadata}
          </span>
        )}
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
                aria-checked={action.role === "menuitemcheckbox" || action.role === "menuitemradio"
                  ? action.checked
                  : undefined}
                data-testid={action.testId}
                disabled={disabled || action.disabled}
                icon={action.icon}
                key={action.id}
                onClick={action.onClick}
                role={action.role}
                size="sm"
                tone={action.tone}
                variant="quiet"
                {...action.testAttributes}
              />
          ))}
        </span>
      )}
    </span>
  );
});
