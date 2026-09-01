import {
  forwardRef,
  type HTMLAttributes,
  type MouseEventHandler,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import { IconButton, type IconButtonProps } from "../IconButton";
import styles from "./ActivityItem.module.css";

export type ActivityItemAppearance = "inline" | "surface";

export interface ActivityItemAction {
  disabled?: boolean;
  icon: ReactNode;
  id: string;
  label: string;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  tone?: IconButtonProps["tone"];
}

export interface ActivityItemProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, "children" | "onClick"> {
  actions?: readonly ActivityItemAction[];
  appearance?: ActivityItemAppearance;
  children: ReactNode;
  detail?: ReactNode;
  disabled?: boolean;
  label?: ReactNode;
  leading?: ReactNode;
  metadata?: ReactNode;
  onActivate?: MouseEventHandler<HTMLButtonElement>;
}

export interface ChangeCountProps extends HTMLAttributes<HTMLSpanElement> {
  additions?: number;
  deletions?: number;
}

function ActivityItemContent({
  children,
  label,
  leading,
}: Pick<ActivityItemProps, "children" | "label" | "leading">) {
  return (
    <>
      {leading !== undefined && leading !== null && (
        <span aria-hidden="true" className={styles.leading} data-bf-part="leading">
          {leading}
        </span>
      )}
      <span className={styles.content} data-bf-part="content">
        {label !== undefined && label !== null && (
          <span className={styles.label} data-bf-part="label">{label}</span>
        )}
        <span className={styles.description} data-bf-part="description">{children}</span>
      </span>
    </>
  );
}

export const ActivityItem = forwardRef<HTMLSpanElement, ActivityItemProps>(
  function ActivityItem({
    actions = [],
    appearance = "inline",
    children,
    className,
    detail,
    disabled = false,
    label,
    leading,
    metadata,
    onActivate,
    ...props
  }, ref) {
    const content = (
      <ActivityItemContent label={label} leading={leading}>
        {children}
      </ActivityItemContent>
    );
    const hasDetail = detail !== undefined && detail !== null;

    return (
      <span
        {...props}
        aria-disabled={disabled || undefined}
        className={classNames(styles.root, className)}
        data-appearance={appearance}
        data-bf-component="activity-item"
        data-disabled={disabled ? "true" : "false"}
        data-has-detail={hasDetail ? "true" : "false"}
        ref={ref}
      >
        {onActivate ? (
          <button
            className={styles.trigger}
            data-bf-part="trigger"
            disabled={disabled}
            onClick={onActivate}
            type="button"
          >
            {content}
          </button>
        ) : (
          <span className={styles.body} data-bf-part="body">{content}</span>
        )}
        {metadata !== undefined && metadata !== null && (
          <span className={styles.metadata} data-bf-part="metadata">{metadata}</span>
        )}
        {actions.length > 0 && (
          <>
            <span aria-hidden="true" className={styles.divider} data-bf-part="divider" />
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
          </>
        )}
        {hasDetail && (
          <span className={styles.detail} data-bf-part="detail">{detail}</span>
        )}
      </span>
    );
  },
);

export const ChangeCount = forwardRef<HTMLSpanElement, ChangeCountProps>(
  function ChangeCount({ additions = 0, className, deletions = 0, ...props }, ref) {
    return (
      <span
        {...props}
        className={classNames(styles.changeCount, className)}
        data-bf-component="change-count"
        ref={ref}
      >
        <span className={styles.additions} data-bf-part="additions">+{Math.abs(additions)}</span>
        <span className={styles.deletions} data-bf-part="deletions">-{Math.abs(deletions)}</span>
      </span>
    );
  },
);
