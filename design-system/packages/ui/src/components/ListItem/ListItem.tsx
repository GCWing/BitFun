import {
  forwardRef,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./ListItem.module.css";

export interface ListItemProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  actions?: ReactNode;
  children: ReactNode;
  leadingIcon?: ReactNode;
  selected?: boolean;
  shortcut?: ReactNode;
  "data-bf-preview-state"?: "active" | "hover";
}

export const ListItem = forwardRef<HTMLButtonElement, ListItemProps>(function ListItem({
  actions,
  children,
  className,
  disabled,
  leadingIcon,
  selected = false,
  shortcut,
  type = "button",
  "data-bf-preview-state": previewState,
  ...props
}, ref) {
  return (
    <span
      className={classNames(styles.root, className)}
      data-bf-component="list-item"
      data-bf-part="root"
      data-bf-preview-state={previewState}
      data-disabled={disabled || undefined}
      data-selected={selected || undefined}
    >
      <button
        {...props}
        className={styles.control}
        data-bf-part="control"
        disabled={disabled}
        ref={ref}
        type={type}
      >
        {leadingIcon && (
          <span aria-hidden="true" className={styles.icon} data-bf-part="leadingIcon">
            {leadingIcon}
          </span>
        )}
        <span className={styles.label} data-bf-part="label">{children}</span>
      </button>
      {actions && (
        <span className={styles.actions} data-bf-part="actions">{actions}</span>
      )}
      {shortcut && (
        <span className={styles.shortcut} data-bf-part="shortcut">{shortcut}</span>
      )}
    </span>
  );
});
