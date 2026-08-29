import { Children, useState, type CSSProperties, type HTMLAttributes, type ReactNode } from "react";
import { classNames } from "../../internal/classNames";
import styles from "./Avatar.module.css";

export type AvatarSize = "sm" | "md" | "lg" | "small" | "medium" | "large" | number;
export interface AvatarProps extends Omit<HTMLAttributes<HTMLSpanElement>, "children"> { alt?: string; children?: ReactNode; icon?: ReactNode; onError?: () => void; shape?: "circle" | "square"; size?: AvatarSize; src?: string; }
export interface AvatarGroupProps extends HTMLAttributes<HTMLDivElement> { children: ReactNode; maxCount?: number; }
function normalizedSize(size: AvatarSize) { return size === "small" ? "sm" : size === "large" ? "lg" : size === "medium" ? "md" : size; }

export function Avatar({ alt = "", children, className, icon, onError, shape = "circle", size = "md", src, style, ...props }: AvatarProps) {
  const [imageFailed, setImageFailed] = useState(false);
  const resolvedSize = normalizedSize(size);
  const customStyle: CSSProperties | undefined = typeof resolvedSize === "number" ? { ...style, blockSize: resolvedSize, inlineSize: resolvedSize } : style;
  return (
    <span {...props} className={classNames(styles.root, className)} data-bf-component="avatar" data-bf-shape={shape} data-size={typeof resolvedSize === "number" ? "custom" : resolvedSize} style={customStyle}>
      {src && !imageFailed
        ? <img alt={alt} className={styles.image} data-bf-part="image" onError={() => { setImageFailed(true); onError?.(); }} src={src} />
        : icon !== undefined
          ? <span className={styles.content} data-bf-part="icon">{icon}</span>
          : <span className={styles.content} data-bf-part="text">{children}</span>}
    </span>
  );
}

export function AvatarGroup({ children, className, maxCount = 5, ...props }: AvatarGroupProps) {
  const items = Children.toArray(children);
  const visible = maxCount > 0 ? items.slice(0, maxCount) : items;
  const hiddenCount = Math.max(0, items.length - visible.length);
  return <div {...props} className={classNames(styles.group, className)} data-bf-component="avatar-group">{visible}{hiddenCount > 0 && <Avatar aria-label={`${hiddenCount} more`}>+{hiddenCount}</Avatar>}</div>;
}
