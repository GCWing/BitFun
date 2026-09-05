import { type HTMLAttributes, type ReactNode } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileMessage.module.css";
export type MobileMessageRole = "user" | "assistant" | "system";
export interface MobileMessageProps extends HTMLAttributes<HTMLElement> { actions?: ReactNode; children: ReactNode; roleType: MobileMessageRole; }
export function MobileMessage({ actions, children, className, roleType, ...props }: MobileMessageProps) {
  return <article {...props} className={classNames(styles.root, className)} data-openbitfun-component="mobile-message" data-role={roleType}><div className={styles.content}>{children}</div>{actions && <div className={styles.actions}>{actions}</div>}</article>;
}
