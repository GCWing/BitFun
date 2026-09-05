import { forwardRef, type HTMLAttributes, type ReactNode } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileSection.module.css";
export interface MobileSectionProps extends Omit<HTMLAttributes<HTMLElement>, "title"> { action?: ReactNode; description?: ReactNode; title?: ReactNode; }
export const MobileSection = forwardRef<HTMLElement, MobileSectionProps>(function MobileSection({ action, children, className, description, title, ...props }, ref) {
  return <section {...props} className={classNames(styles.root, className)} data-openbitfun-component="mobile-section" ref={ref}>{(title || action || description) && <header className={styles.header}><div className={styles.copy}>{title && <h2 className={styles.title}>{title}</h2>}{description && <p className={styles.description}>{description}</p>}</div>{action && <div className={styles.action}>{action}</div>}</header>}<div className={styles.body}>{children}</div></section>;
});
