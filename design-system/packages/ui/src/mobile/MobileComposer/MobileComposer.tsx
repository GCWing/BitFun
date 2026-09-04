import {
  forwardRef,
  type HTMLAttributes,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./MobileComposer.module.css";

export interface MobileComposerProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "onActivate"> {
  "aria-label"?: string;
  children: ReactNode;
  endActions?: ReactNode;
  expanded?: boolean;
  leading?: ReactNode;
  onActivate?: () => void;
  startActions?: ReactNode;
}

export const MobileComposer = forwardRef<HTMLDivElement, MobileComposerProps>(
  function MobileComposer({
    "aria-label": ariaLabel,
    children,
    className,
    endActions,
    expanded = false,
    leading,
    onActivate,
    onKeyDown,
    startActions,
    ...props
  }, ref) {
    const interactive = !expanded && Boolean(onActivate);

    const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
      onKeyDown?.(event);
      if (event.defaultPrevented || !interactive) return;
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        onActivate?.();
      }
    };

    return (
      <div
        {...props}
        className={classNames(styles.root, className)}
        data-bf-component="mobile-composer"
        data-expanded={expanded ? "true" : "false"}
        data-interactive={interactive ? "true" : "false"}
        ref={ref}
      >
        {leading !== undefined && leading !== null && (
          <div className={styles.leading} data-bf-part="leading">{leading}</div>
        )}
        <div
          aria-label={interactive ? ariaLabel : undefined}
          className={styles.editor}
          data-bf-part="editor"
          onClick={interactive ? onActivate : undefined}
          onKeyDown={handleKeyDown}
          role={interactive ? "button" : undefined}
          tabIndex={interactive ? 0 : undefined}
        >
          {children}
        </div>
        <div className={styles.toolbar} data-bf-part="toolbar">
          <div className={styles.startActions} data-bf-part="start-actions">
            {startActions}
          </div>
          <div className={styles.endActions} data-bf-part="end-actions">
            {endActions}
          </div>
        </div>
      </div>
    );
  },
);
