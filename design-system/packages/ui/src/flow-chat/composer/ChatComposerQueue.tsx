import {
  forwardRef,
  type HTMLAttributes,
  type LiHTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./ChatComposerQueue.module.css";

export type ChatComposerQueueItemState =
  | "default"
  | "sending"
  | "failed";

export interface ChatComposerQueueProps
  extends HTMLAttributes<HTMLElement> {}

export interface ChatComposerQueueHeaderProps
  extends HTMLAttributes<HTMLDivElement> {}

export interface ChatComposerQueueTitleProps
  extends HTMLAttributes<HTMLSpanElement> {
  count?: ReactNode;
}

export interface ChatComposerQueueListProps
  extends HTMLAttributes<HTMLUListElement> {}

export interface ChatComposerQueueItemProps
  extends LiHTMLAttributes<HTMLLIElement> {
  state?: ChatComposerQueueItemState;
}

export interface ChatComposerQueueItemContentProps
  extends HTMLAttributes<HTMLDivElement> {}

export interface ChatComposerQueueAttachmentBadgeProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, "children"> {
  count: number;
  label: string;
}

export interface ChatComposerQueueItemActionsProps
  extends HTMLAttributes<HTMLDivElement> {}

/** Neutral queue surface rendered inside a ChatComposer body. */
export const ChatComposerQueue = forwardRef<HTMLElement, ChatComposerQueueProps>(
  function ChatComposerQueue({ className, ...props }, ref) {
    return (
      <section
        {...props}
        className={classNames(styles.root, className)}
        data-openbitfun-component="chat-composer-queue"
        data-openbitfun-part="root"
        ref={ref}
      />
    );
  },
);

/** Queue heading row. Supply a real icon from the consumer's icon library. */
export const ChatComposerQueueHeader = forwardRef<
  HTMLDivElement,
  ChatComposerQueueHeaderProps
>(function ChatComposerQueueHeader({ className, ...props }, ref) {
  return (
    <div
      {...props}
      className={classNames(styles.header, className)}
      data-openbitfun-component="chat-composer-queue"
      data-openbitfun-part="header"
      ref={ref}
    />
  );
});

/** Queue label and its separate total-count fact. */
export const ChatComposerQueueTitle = forwardRef<
  HTMLSpanElement,
  ChatComposerQueueTitleProps
>(function ChatComposerQueueTitle({ children, className, count, ...props }, ref) {
  return (
    <span
      {...props}
      className={classNames(styles.title, className)}
      data-openbitfun-component="chat-composer-queue"
      data-openbitfun-part="title"
      ref={ref}
    >
      <span className={styles.titleLabel}>{children}</span>
      {count !== undefined && count !== null && (
        <span
          className={styles.totalCount}
          data-openbitfun-component="chat-composer-queue"
          data-openbitfun-part="totalCount"
        >
          {count}
        </span>
      )}
    </span>
  );
});

export const ChatComposerQueueList = forwardRef<
  HTMLUListElement,
  ChatComposerQueueListProps
>(function ChatComposerQueueList({ className, ...props }, ref) {
  return (
    <ul
      {...props}
      className={classNames(styles.list, className)}
      data-openbitfun-component="chat-composer-queue"
      data-openbitfun-part="list"
      ref={ref}
    />
  );
});

export const ChatComposerQueueItem = forwardRef<
  HTMLLIElement,
  ChatComposerQueueItemProps
>(function ChatComposerQueueItem({ className, state = "default", ...props }, ref) {
  return (
    <li
      {...props}
      className={classNames(styles.item, className)}
      data-openbitfun-component="chat-composer-queue"
      data-openbitfun-part="item"
      data-openbitfun-state={state === "default" ? undefined : state}
      ref={ref}
    />
  );
});

export const ChatComposerQueueItemContent = forwardRef<
  HTMLDivElement,
  ChatComposerQueueItemContentProps
>(function ChatComposerQueueItemContent({ className, ...props }, ref) {
  return (
    <div
      {...props}
      className={classNames(styles.content, className)}
      data-openbitfun-component="chat-composer-queue"
      data-openbitfun-part="content"
      ref={ref}
    />
  );
});

/** Per-message attachment count; this is intentionally distinct from queue position. */
export const ChatComposerQueueAttachmentBadge = forwardRef<
  HTMLSpanElement,
  ChatComposerQueueAttachmentBadgeProps
>(function ChatComposerQueueAttachmentBadge({ className, count, label, ...props }, ref) {
  if (!Number.isFinite(count) || count <= 0) {
    return null;
  }

  return (
    <span
      {...props}
      aria-label={label}
      className={classNames(styles.attachmentBadge, className)}
      data-openbitfun-component="chat-composer-queue"
      data-openbitfun-part="attachmentCount"
      ref={ref}
      title={label}
    >
      {Math.trunc(count)}
    </span>
  );
});

export const ChatComposerQueueItemActions = forwardRef<
  HTMLDivElement,
  ChatComposerQueueItemActionsProps
>(function ChatComposerQueueItemActions({ className, ...props }, ref) {
  return (
    <div
      {...props}
      className={classNames(styles.actions, className)}
      data-openbitfun-component="chat-composer-queue"
      data-openbitfun-part="actions"
      ref={ref}
    />
  );
});
