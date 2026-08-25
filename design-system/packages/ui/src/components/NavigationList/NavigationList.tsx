import {
  forwardRef,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./NavigationList.module.css";

export interface NavigationListProps extends HTMLAttributes<HTMLElement> {
  footer?: ReactNode;
  header?: ReactNode;
}

export interface NavigationListSectionProps extends Omit<HTMLAttributes<HTMLDivElement>, "title"> {
  actions?: ReactNode;
  title?: ReactNode;
}

export const NavigationList = forwardRef<HTMLElement, NavigationListProps>(
  function NavigationList({ children, className, footer, header, ...props }, ref) {
    return (
      <nav
        {...props}
        className={classNames(styles.root, className)}
        data-bf-component="navigation-list"
        ref={ref}
      >
        {header && <div className={styles.header} data-bf-part="header">{header}</div>}
        <div className={styles.content} data-bf-part="content">{children}</div>
        {footer && <div className={styles.footer} data-bf-part="footer">{footer}</div>}
      </nav>
    );
  },
);

export const NavigationListSection = forwardRef<HTMLDivElement, NavigationListSectionProps>(
  function NavigationListSection({ actions, children, className, title, ...props }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.section, className)}
        data-bf-component="navigation-list-section"
        ref={ref}
      >
        {(title || actions) && (
          <div className={styles.sectionHeading} data-bf-part="section-heading">
            <span className={styles.sectionTitle}>{title}</span>
            {actions && <span className={styles.sectionActions}>{actions}</span>}
          </div>
        )}
        <div className={styles.items} data-bf-part="items">{children}</div>
      </div>
    );
  },
);
