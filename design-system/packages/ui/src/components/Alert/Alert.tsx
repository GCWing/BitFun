import { forwardRef, useState, type HTMLAttributes, type ReactNode } from "react";
import { classNames } from "../../internal/classNames";
import { Icon, type IconName } from "../Icon";
import { IconButton } from "../IconButton";
import styles from "./Alert.module.css";

export type AlertTone = "success" | "error" | "warning" | "info";

export interface AlertProps extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "title"> {
  closable?: boolean;
  closeLabel?: string;
  description?: ReactNode;
  message: ReactNode;
  onClose?: () => void;
  showIcon?: boolean;
  title?: ReactNode;
  tone?: AlertTone;
}

const toneIcons: Record<AlertTone, IconName> = {
  error: "info",
  info: "info",
  success: "check-circle",
  warning: "info",
};

export const Alert = forwardRef<HTMLDivElement, AlertProps>(function Alert({
  "aria-live": ariaLive,
  className,
  closable = false,
  closeLabel = "Close",
  description,
  message,
  onClose,
  showIcon = true,
  title,
  tone = "info",
  ...props
}, ref) {
  const [visible, setVisible] = useState(true);
  if (!visible) return null;

  return (
    <div
      {...props}
      aria-live={ariaLive ?? (tone === "error" ? "assertive" : "polite")}
      className={classNames(styles.root, className)}
      data-openbitfun-component="alert"
      data-openbitfun-tone={tone}
      ref={ref}
      role="alert"
    >
      {showIcon && <span className={styles.icon} data-openbitfun-part="icon"><Icon name={toneIcons[tone]} size="sm" /></span>}
      <span className={styles.content} data-openbitfun-part="content">
        {title !== undefined && <span className={styles.title} data-openbitfun-part="title">{title}</span>}
        <span className={styles.message} data-openbitfun-part="message">{message}</span>
        {description !== undefined && <span className={styles.description} data-openbitfun-part="description">{description}</span>}
      </span>
      {closable && (
        <IconButton
          aria-label={closeLabel}
          className={styles.close}
          icon={<Icon name="xmark" />}
          onClick={() => { setVisible(false); onClose?.(); }}
          size="xs"
          variant="quiet"
        />
      )}
    </div>
  );
});
