import {
  forwardRef,
  useId,
  type HTMLAttributes,
  type MouseEventHandler,
  type ReactNode,
  type Ref,
} from "react";
import { ActionItem, type ActionItemProps } from "../ActionItem";
import { IconButton, type IconButtonProps } from "../IconButton";
import { ScrollArea, type ScrollbarVisibility } from "../ScrollArea";
import { classNames } from "../../internal/classNames";
import styles from "./NavigationPanel.module.css";

export interface NavigationPanelProps
  extends Omit<HTMLAttributes<HTMLElement>, "children"> {
  bodyClassName?: string;
  /** Ref to the scroll viewport so product sticky headers can track the same root. */
  bodyRef?: Ref<HTMLDivElement>;
  children: ReactNode;
  contentClassName?: string;
  footer?: ReactNode;
  header?: ReactNode;
  scrollbarVisibility?: ScrollbarVisibility;
}

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
  itemsClassName?: string;
  title?: ReactNode;
}

export type NavigationPanelSeparatorProps = HTMLAttributes<HTMLDivElement>;

export const NavigationPanel = forwardRef<HTMLElement, NavigationPanelProps>(
  function NavigationPanel({
    bodyClassName,
    bodyRef,
    children,
    className,
    contentClassName,
    footer,
    header,
    scrollbarVisibility = "auto",
    ...props
  }, ref) {
    return (
      <nav
        {...props}
        className={classNames(styles.root, className)}
        data-bf-component="navigation-panel"
        ref={ref}
      >
        {header !== undefined && header !== null && (
          <div className={styles.header} data-bf-part="header">{header}</div>
        )}
        <ScrollArea
          className={classNames(styles.body, bodyClassName)}
          orientation="vertical"
          ref={bodyRef}
          scrollbarVisibility={scrollbarVisibility}
        >
          <div
            className={classNames(styles.content, contentClassName)}
            data-bf-part="content"
          >
            {children}
          </div>
        </ScrollArea>
        {footer !== undefined && footer !== null && (
          <div className={styles.footer} data-bf-part="footer">{footer}</div>
        )}
      </nav>
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
    itemsClassName,
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
        data-bf-part="section"
        ref={ref}
      >
        {headingId && (
          <div className={styles.heading} data-bf-part="heading" id={headingId}>
            <span className={styles.headingLabel} data-bf-part="heading-label">
              {title}
            </span>
            {actions.length > 0 && (
              <span className={styles.headingActions} data-bf-part="heading-actions">
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
        <div
          className={classNames(styles.items, itemsClassName)}
          data-bf-part="section-items"
        >
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
        data-bf-part="separator"
        ref={ref}
        role="separator"
      />
    );
  },
);
