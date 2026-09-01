import {
  forwardRef,
  type ButtonHTMLAttributes,
  type MouseEventHandler,
  type ReactNode,
} from "react";
import { IconButton, type IconButtonProps } from "../IconButton";
import { classNames } from "../../internal/classNames";
import styles from "./ActionCard.module.css";

export interface ActionCardAction {
  disabled?: boolean;
  icon: ReactNode;
  id: string;
  label: string;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  tone?: IconButtonProps["tone"];
}

export type ActionCardSize = "sm" | "md";

export interface ActionCardProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children" | "className"> {
  actions?: readonly ActionCardAction[];
  children: ReactNode;
  className?: string;
  description?: ReactNode;
  leading?: ReactNode;
  selected?: boolean;
  size?: ActionCardSize;
}

export const ActionCard = forwardRef<HTMLButtonElement, ActionCardProps>(
  function ActionCard({
    actions = [],
    children,
    className,
    description,
    disabled = false,
    leading,
    selected = false,
    size = "sm",
    type = "button",
    ...props
  }, ref) {
    return (
      <span
        className={classNames(styles.root, className)}
        data-bf-component="action-card"
        data-disabled={disabled ? "true" : "false"}
        data-has-actions={actions.length > 0 ? "true" : "false"}
        data-selected={selected ? "true" : "false"}
        data-size={size}
      >
        <button
          {...props}
          className={styles.trigger}
          data-bf-part="trigger"
          disabled={disabled}
          ref={ref}
          type={type}
        >
          {leading !== undefined && leading !== null && (
            <span aria-hidden="true" className={styles.leading} data-bf-part="leading">
              {leading}
            </span>
          )}
          <span className={styles.content} data-bf-part="content">
            <span className={styles.title} data-bf-part="title">{children}</span>
            {description !== undefined && description !== null && (
              <span className={styles.description} data-bf-part="description">
                {description}
              </span>
            )}
          </span>
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
                size="xs"
                tone={action.tone}
                variant="quiet"
              />
            ))}
          </span>
        )}
      </span>
    );
  },
);
