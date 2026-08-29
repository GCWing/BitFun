import {
  forwardRef,
  type CSSProperties,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { classNames } from "../../internal/classNames";
import { Icon } from "../Icon";
import styles from "./Empty.module.css";

export type EmptyMediaSize = "sm" | "md" | "lg" | "small" | "medium" | "large" | number;

export interface EmptyProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "title"> {
  actions?: ReactNode;
  children?: ReactNode;
  description?: ReactNode;
  icon?: ReactNode;
  image?: ReactNode;
  imageSize?: EmptyMediaSize;
  title?: ReactNode;
}

function normalizeMediaSize(size: EmptyMediaSize) {
  if (size === "small") return "sm";
  if (size === "large") return "lg";
  if (size === "medium") return "md";
  return size;
}

export const Empty = forwardRef<HTMLDivElement, EmptyProps>(function Empty({
  actions,
  children,
  className,
  description,
  icon,
  image,
  imageSize = "md",
  style,
  title,
  ...props
}, ref) {
  const resolvedSize = normalizeMediaSize(imageSize);
  const media = icon ?? image ?? <Icon name="folder" size="lg" tone="muted" />;
  const mediaStyle = typeof resolvedSize === "number"
    ? { blockSize: resolvedSize, inlineSize: resolvedSize } as CSSProperties
    : undefined;
  const footer = actions ?? children;

  return (
    <div
      {...props}
      className={classNames(styles.root, className)}
      data-bf-component="empty"
      ref={ref}
      style={style}
    >
      <div
        className={styles.media}
        data-bf-part="media"
        data-size={typeof resolvedSize === "number" ? "custom" : resolvedSize}
        style={mediaStyle}
      >
        {media}
      </div>
      {title !== undefined && title !== null && (
        <div className={styles.title} data-bf-part="title">{title}</div>
      )}
      {description !== undefined && description !== null && (
        <div className={styles.description} data-bf-part="description">{description}</div>
      )}
      {footer !== undefined && footer !== null && (
        <div className={styles.actions} data-bf-part="actions">{footer}</div>
      )}
    </div>
  );
});
