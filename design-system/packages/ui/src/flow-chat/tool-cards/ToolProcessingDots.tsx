import { classNames } from "../../internal/classNames";
import styles from "./ToolProcessingDots.module.css";

export type ToolProcessingDotsSize = 10 | 12 | 14 | 16;

export interface ToolProcessingDotsProps {
  className?: string;
  size?: ToolProcessingDotsSize;
}

export function ToolProcessingDots({
  className,
  size = 14,
}: ToolProcessingDotsProps) {
  return (
    <span
      aria-hidden="true"
      className={classNames(styles.root, className)}
      data-bf-component="flow-chat-tool-card"
      data-bf-part="processing"
      data-size={size}
      role="presentation"
    >
      <span className={styles.dot} />
      <span className={styles.dot} />
      <span className={styles.dot} />
    </span>
  );
}
