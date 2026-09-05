import { type HTMLAttributes, type ReactNode } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileStatus.module.css";
export type MobileStatusTone = "neutral" | "info" | "danger";
export interface MobileStatusProps extends Omit<HTMLAttributes<HTMLDivElement>, "title"> { action?: ReactNode; description?: ReactNode; icon?: ReactNode; loading?: boolean; title?: ReactNode; tone?: MobileStatusTone; }
export function MobileStatus({ action, className, description, icon, loading = false, role, title, tone = "neutral", ...props }: MobileStatusProps) {
  return <div {...props} aria-busy={loading || undefined} className={classNames(styles.root, className)} data-openbitfun-component="mobile-status" data-tone={tone} role={role ?? (tone === "danger" ? "alert" : "status")}>{loading ? <span aria-hidden="true" className={styles.spinner} /> : icon && <span className={styles.icon}>{icon}</span>}{title && <div className={styles.title}>{title}</div>}{description && <div className={styles.description}>{description}</div>}{action && <div className={styles.action}>{action}</div>}</div>;
}
