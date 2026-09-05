import { type HTMLAttributes, type ReactNode } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileBanner.module.css";
export type MobileBannerTone = "neutral" | "info" | "warning" | "danger";
export interface MobileBannerProps extends HTMLAttributes<HTMLDivElement> { action?: ReactNode; icon?: ReactNode; tone?: MobileBannerTone; }
export function MobileBanner({ action, children, className, icon, role, tone = "neutral", ...props }: MobileBannerProps) {
  return <div {...props} className={classNames(styles.root, className)} data-openbitfun-component="mobile-banner" data-tone={tone} role={role ?? (tone === "danger" ? "alert" : "status")}>{icon && <span className={styles.icon}>{icon}</span>}<div className={styles.content}>{children}</div>{action && <div className={styles.action}>{action}</div>}</div>;
}
