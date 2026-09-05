import { type HTMLAttributes, type ReactNode } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileBadge.module.css";
export type MobileBadgeTone = "neutral" | "info" | "success" | "warning" | "danger";
export interface MobileBadgeProps extends HTMLAttributes<HTMLSpanElement> { dot?: boolean; leading?: ReactNode; tone?: MobileBadgeTone; }
export function MobileBadge({ children, className, dot = false, leading, tone = "neutral", ...props }: MobileBadgeProps) {
  return <span {...props} className={classNames(styles.root, className)} data-openbitfun-component="mobile-badge" data-tone={tone}>{dot && <span aria-hidden="true" className={styles.dot} />}{leading}<span>{children}</span></span>;
}
