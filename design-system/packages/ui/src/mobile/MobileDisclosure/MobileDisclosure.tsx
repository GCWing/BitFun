import { type HTMLAttributes, type ReactNode } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileDisclosure.module.css";
export interface MobileDisclosureProps extends Omit<HTMLAttributes<HTMLDivElement>, "onToggle" | "title"> { children: ReactNode; disabled?: boolean; leading?: ReactNode; metadata?: ReactNode; onToggle: () => void; open: boolean; title: ReactNode; }
export function MobileDisclosure({ children, className, disabled, leading, metadata, onToggle, open, title, ...props }: MobileDisclosureProps) {
  return (
    <div {...props} className={classNames(styles.root, className)} data-openbitfun-component="mobile-disclosure" data-open={open ? "true" : "false"}>
      <button aria-expanded={open} className={styles.trigger} data-openbitfun-part="trigger" disabled={disabled} onClick={onToggle} type="button">
        {leading && <span className={styles.leading} data-openbitfun-part="leading">{leading}</span>}
        <span className={styles.title} data-openbitfun-part="title">{title}</span>
        {metadata && <span className={styles.metadata} data-openbitfun-part="metadata">{metadata}</span>}
        <svg aria-hidden="true" className={styles.chevron} data-openbitfun-part="chevron" viewBox="0 0 16 16">
          <path d="M6 4l4 4-4 4" fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" />
        </svg>
      </button>
      {open && <div className={styles.body} data-openbitfun-part="body">{children}</div>}
    </div>
  );
}
