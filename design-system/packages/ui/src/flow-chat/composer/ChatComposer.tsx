import {
  Children,
  forwardRef,
  isValidElement,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import {
  IconButton,
  type IconButtonProps,
} from "../../components/IconButton";
import { classNames } from "../../internal/classNames";
import styles from "./ChatComposer.module.css";

export type ChatComposerLayout = "compact" | "expanded";

export interface ChatComposerProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  busy?: boolean;
  children: ReactNode;
  contextBar?: ReactNode;
  disabled?: boolean;
  endActions?: ReactNode;
  layout?: ChatComposerLayout;
  queue?: ReactNode;
  startActions?: ReactNode;
}

export interface ChatComposerSlotProps {
  children: ReactNode;
}

export type ChatComposerActionButtonProps = Omit<
  IconButtonProps,
  "shape" | "size"
>;

/** Stable icon action for ChatComposer start and end action tracks. */
export const ChatComposerActionButton = forwardRef<
  HTMLButtonElement,
  ChatComposerActionButtonProps
>(function ChatComposerActionButton({ className, ...props }, ref) {
  return (
    <IconButton
      {...props}
      className={classNames(styles.actionButton, className)}
      data-openbitfun-role="composer-action"
      ref={ref}
      shape="circle"
      size="sm"
    />
  );
});

/** Compound slot for complex consumers that keep content and actions adjacent. */
export function ChatComposerContent({ children }: ChatComposerSlotProps) {
  return <>{children}</>;
}

/** Compound slot for controls at the leading edge of the composer. */
export function ChatComposerStartActions({ children }: ChatComposerSlotProps) {
  return <>{children}</>;
}

/** Compound slot for controls at the trailing edge of the composer. */
export function ChatComposerEndActions({ children }: ChatComposerSlotProps) {
  return <>{children}</>;
}

function hasSlot(value: ReactNode): boolean {
  if (Array.isArray(value)) return value.some(hasSlot);
  return value !== undefined && value !== null && value !== false;
}

function resolveCompoundSlots(children: ReactNode) {
  const content: ReactNode[] = [];
  const startActions: ReactNode[] = [];
  const endActions: ReactNode[] = [];

  Children.toArray(children).forEach((child) => {
    if (isValidElement<ChatComposerSlotProps>(child)) {
      if (child.type === ChatComposerContent) {
        content.push(...Children.toArray(child.props.children));
        return;
      }
      if (child.type === ChatComposerStartActions) {
        startActions.push(...Children.toArray(child.props.children));
        return;
      }
      if (child.type === ChatComposerEndActions) {
        endActions.push(...Children.toArray(child.props.children));
        return;
      }
    }
    content.push(child);
  });

  return {
    content,
    endActions,
    startActions,
  };
}

/**
 * Theme-independent FlowChat composer anatomy.
 *
 * Product state, localization, rich-text behavior, menus, and host actions are
 * supplied through slots. The package owns the stable context-bar/surface
 * geometry and the responsive compact/expanded layout.
 */
export const ChatComposer = forwardRef<HTMLDivElement, ChatComposerProps>(
  function ChatComposer({
    "aria-busy": ariaBusy,
    busy = false,
    children,
    className,
    contextBar,
    disabled = false,
    endActions,
    layout = "compact",
    queue,
    startActions,
    ...props
  }, ref) {
    const contextVisible = hasSlot(contextBar);
    const compoundSlots = resolveCompoundSlots(children);
    const resolvedStartActions = hasSlot(startActions)
      ? startActions
      : compoundSlots.startActions;
    const resolvedEndActions = hasSlot(endActions)
      ? endActions
      : compoundSlots.endActions;
    const state = [busy && "busy", disabled && "disabled"]
      .filter(Boolean)
      .join(" ") || undefined;

    return (
      <div
        {...props}
        aria-busy={busy ? true : ariaBusy}
        className={classNames(styles.root, className)}
        data-openbitfun-component="chat-composer"
        data-openbitfun-state={state}
        data-has-context={contextVisible ? "true" : "false"}
        ref={ref}
      >
        {contextVisible && (
          <div className={styles.context} data-openbitfun-part="contextBar">
            {contextBar}
          </div>
        )}
        <div className={styles.body} data-openbitfun-part="body">
          {hasSlot(queue) && (
            <div className={styles.queue} data-openbitfun-part="queue">
              {queue}
            </div>
          )}
          <div
            aria-disabled={disabled || undefined}
            className={styles.surface}
            data-openbitfun-layout={layout}
            data-openbitfun-part="surface"
          >
            {hasSlot(resolvedStartActions) && (
              <div className={styles.startActions} data-openbitfun-part="startActions">
                {resolvedStartActions}
              </div>
            )}
            <div className={styles.content} data-openbitfun-part="content">
              {compoundSlots.content}
            </div>
            {hasSlot(resolvedEndActions) && (
              <div className={styles.endActions} data-openbitfun-part="endActions">
                {resolvedEndActions}
              </div>
            )}
          </div>
        </div>
      </div>
    );
  },
);
