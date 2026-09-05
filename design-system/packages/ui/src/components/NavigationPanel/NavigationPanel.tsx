import {
  forwardRef,
  useId,
  type HTMLAttributes,
  type MouseEventHandler,
  type ReactNode,
} from "react";
import { ActionItem, type ActionItemProps } from "../ActionItem";
import { IconButton, type IconButtonProps } from "../IconButton";
import { ScrollArea, type ScrollAreaProps } from "../ScrollArea";
import { classNames } from "../../internal/classNames";
import styles from "./NavigationPanel.module.css";

export interface NavigationPanelProps
  extends Omit<HTMLAttributes<HTMLElement>, "children"> {
  children: ReactNode;
}

export type NavigationPanelHeaderProps = HTMLAttributes<HTMLDivElement>;

export interface NavigationPanelBodyProps
  extends Omit<ScrollAreaProps, "orientation"> {}

export type NavigationPanelContentProps = HTMLAttributes<HTMLDivElement>;
export type NavigationPanelFooterProps = HTMLAttributes<HTMLDivElement>;

export interface NavigationPanelItemProps extends ActionItemProps {
  selected?: boolean;
}

export interface NavigationPanelSectionAction {
  disabled?: boolean;
  icon: ReactNode;
  id: string;
  label: string;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  tone?: IconButtonProps["tone"];
}

export interface NavigationPanelSectionProps
  extends Omit<HTMLAttributes<HTMLElement>, "title"> {
  actions?: readonly NavigationPanelSectionAction[];
  children: ReactNode;
  title?: ReactNode;
}

export type NavigationPanelSeparatorProps = HTMLAttributes<HTMLDivElement>;

export const NavigationPanel = forwardRef<HTMLElement, NavigationPanelProps>(
  function NavigationPanel({
    children,
    className,
    ...props
  }, ref) {
    return (
      <nav
        {...props}
        className={classNames(styles.root, className)}
        data-openbitfun-component="navigation-panel"
        ref={ref}
      >
        {children}
      </nav>
    );
  },
);

export const NavigationPanelHeader = forwardRef<HTMLDivElement, NavigationPanelHeaderProps>(
  function NavigationPanelHeader({ className, ...props }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.header, className)}
        data-openbitfun-part="header"
        ref={ref}
      />
    );
  },
);

export const NavigationPanelBody = forwardRef<HTMLDivElement, NavigationPanelBodyProps>(
  function NavigationPanelBody({ className, scrollbarVisibility = "auto", ...props }, ref) {
    return (
      <ScrollArea
        {...props}
        className={classNames(styles.body, className)}
        data-openbitfun-part="body"
        orientation="vertical"
        ref={ref}
        scrollbarVisibility={scrollbarVisibility}
      />
    );
  },
);

export const NavigationPanelContent = forwardRef<HTMLDivElement, NavigationPanelContentProps>(
  function NavigationPanelContent({ className, ...props }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.content, className)}
        data-openbitfun-part="content"
        ref={ref}
      />
    );
  },
);

export const NavigationPanelFooter = forwardRef<HTMLDivElement, NavigationPanelFooterProps>(
  function NavigationPanelFooter({ className, ...props }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.footer, className)}
        data-openbitfun-part="footer"
        ref={ref}
      />
    );
  },
);

export const NavigationPanelItem = forwardRef<HTMLButtonElement, NavigationPanelItemProps>(
  function NavigationPanelItem({
    "aria-current": ariaCurrent,
    className,
    selected = false,
    ...props
  }, ref) {
    return (
      <ActionItem
        {...props}
        aria-current={ariaCurrent ?? (selected ? "page" : undefined)}
        className={classNames(styles.item, className)}
        ref={ref}
      />
    );
  },
);

export const NavigationPanelSection = forwardRef<HTMLElement, NavigationPanelSectionProps>(
  function NavigationPanelSection({
    "aria-label": ariaLabel,
    "aria-labelledby": ariaLabelledBy,
    actions = [],
    children,
    className,
    title,
    ...props
  }, ref) {
    const generatedHeadingId = useId();
    const headingId = title !== undefined && title !== null ? generatedHeadingId : undefined;
    const resolvedLabelledBy = ariaLabel || ariaLabelledBy ? ariaLabelledBy : headingId;

    return (
      <section
        {...props}
        aria-label={ariaLabel}
        aria-labelledby={resolvedLabelledBy}
        className={classNames(styles.section, className)}
        data-openbitfun-part="section"
        ref={ref}
      >
        {headingId && (
          <div className={styles.heading} data-openbitfun-part="heading" id={headingId}>
            <span className={styles.headingLabel} data-openbitfun-part="heading-label">
              {title}
            </span>
            {actions.length > 0 && (
              <span className={styles.headingActions} data-openbitfun-part="heading-actions">
                {actions.map((action) => (
                  <IconButton
                    aria-label={action.label}
                    className={styles.headingAction}
                    disabled={action.disabled}
                    icon={action.icon}
                    key={action.id}
                    onClick={action.onClick}
                    size="sm"
                    tone={action.tone}
                    variant="quiet"
                  />
                ))}
              </span>
            )}
          </div>
        )}
        <div className={styles.items} data-openbitfun-part="section-items">
          {children}
        </div>
      </section>
    );
  },
);

export const NavigationPanelSeparator = forwardRef<HTMLDivElement, NavigationPanelSeparatorProps>(
  function NavigationPanelSeparator({ className, ...props }, ref) {
    return (
      <div
        {...props}
        className={classNames(styles.separator, className)}
        data-openbitfun-part="separator"
        ref={ref}
        role="separator"
      />
    );
  },
);
