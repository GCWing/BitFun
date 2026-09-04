import {
  forwardRef,
  useId,
  useState,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import { Icon } from "../Icon";
import styles from "./Disclosure.module.css";

export interface DisclosureProps
  extends Omit<HTMLAttributes<HTMLElement>, "children" | "onToggle" | "title"> {
  actions?: ReactNode;
  children: ReactNode;
  defaultOpen?: boolean;
  description?: ReactNode;
  disabled?: boolean;
  leading?: ReactNode;
  onOpenChange?: (open: boolean) => void;
  open?: boolean;
  summary: ReactNode;
}

type InertContentAttributes = HTMLAttributes<HTMLDivElement> & { inert?: "" };

export const Disclosure = forwardRef<HTMLElement, DisclosureProps>(
  function Disclosure({
    actions,
    children,
    className,
    defaultOpen = false,
    description,
    disabled = false,
    leading,
    onOpenChange,
    open,
    summary,
    ...props
  }, ref) {
    const generatedId = useId();
    const triggerId = `openbitfun-disclosure-${generatedId}-trigger`;
    const contentId = `openbitfun-disclosure-${generatedId}-content`;
    const [uncontrolledOpen, setUncontrolledOpen] = useState(defaultOpen);
    const resolvedOpen = open ?? uncontrolledOpen;
    const inertContentAttributes: InertContentAttributes = resolvedOpen ? {} : { inert: "" };

    function toggle() {
      if (disabled) return;
      const nextOpen = !resolvedOpen;
      if (open === undefined) setUncontrolledOpen(nextOpen);
      onOpenChange?.(nextOpen);
    }

    return (
      <section
        {...props}
        className={classNames(styles.root, className)}
        data-openbitfun-component="disclosure"
        data-disabled={disabled ? "true" : "false"}
        data-open={resolvedOpen ? "true" : "false"}
        ref={ref}
      >
        <div className={styles.header} data-openbitfun-part="header">
          <button
            aria-controls={contentId}
            aria-expanded={resolvedOpen}
            className={styles.trigger}
            disabled={disabled}
            id={triggerId}
            onClick={toggle}
            type="button"
          >
            <span aria-hidden="true" className={styles.indicator} data-openbitfun-part="indicator">
              <Icon name="chevron-right" size="sm" />
            </span>
            {leading !== undefined && leading !== null && (
              <span aria-hidden="true" className={styles.leading} data-openbitfun-part="leading">
                {leading}
              </span>
            )}
            <span className={styles.heading} data-openbitfun-part="heading">
              <span className={styles.summary} data-openbitfun-part="summary">{summary}</span>
              {description !== undefined && description !== null && (
                <span className={styles.description} data-openbitfun-part="description">
                  {description}
                </span>
              )}
            </span>
          </button>
          {actions !== undefined && actions !== null && (
            <span className={styles.actions} data-openbitfun-part="actions">{actions}</span>
          )}
        </div>
        <div
          {...inertContentAttributes}
          aria-hidden={!resolvedOpen}
          aria-labelledby={triggerId}
          className={styles.content}
          data-openbitfun-part="content"
          id={contentId}
          role="region"
        >
          <div className={styles.contentInner} data-openbitfun-part="content-inner">
            {children}
          </div>
        </div>
      </section>
    );
  },
);
