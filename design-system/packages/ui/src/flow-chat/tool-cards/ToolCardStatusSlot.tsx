import { Check, Clock, X, type LucideProps } from "lucide-react";
import { classNames } from "../../internal/classNames";
import type { FlowChatToolStatus } from "./FlowChatToolCard";
import {
  ToolProcessingDots,
  type ToolProcessingDotsSize,
} from "./ToolProcessingDots";
import styles from "./ToolCardStatusSlot.module.css";

export interface ToolCardStatusSlotProps {
  className?: string;
  defaultIcon?: "status" | "tool";
  size?: ToolProcessingDotsSize;
  status: FlowChatToolStatus;
  toolIcon?: React.ReactNode;
}

export function hasVisibleToolCardStatusGlyph(status: FlowChatToolStatus): boolean {
  return status !== "cancelled" && status !== "rejected";
}

function StatusGlyph({
  size,
  status,
}: {
  size: ToolProcessingDotsSize;
  status: FlowChatToolStatus;
}) {
  const iconProps: LucideProps = {
    "aria-hidden": true,
    size,
  };

  switch (status) {
    case "completed":
    case "confirmed":
      return <Check {...iconProps} className={styles.success} />;
    case "error":
      return <X {...iconProps} className={styles.danger} />;
    case "queued":
    case "waiting":
      return <Clock {...iconProps} className={styles.muted} />;
    default:
      return <ToolProcessingDots size={size} />;
  }
}

export function ToolCardStatusSlot({
  className,
  defaultIcon = "status",
  size = 16,
  status,
  toolIcon,
}: ToolCardStatusSlotProps) {
  const hasStatusGlyph = hasVisibleToolCardStatusGlyph(status);
  const hasToolIcon = toolIcon !== undefined
    && toolIcon !== null
    && toolIcon !== false
    && toolIcon !== "";

  if (!hasStatusGlyph && !hasToolIcon) {
    return null;
  }

  const resolvedDefaultIcon = hasStatusGlyph ? defaultIcon : "tool";

  return (
    <span
      className={classNames(styles.root, className)}
      data-openbitfun-component="flow-chat-tool-card"
      data-openbitfun-part="statusSlot"
      data-default-icon={resolvedDefaultIcon}
    >
      {hasStatusGlyph && (
        <span
          className={styles.statusLayer}
          data-openbitfun-component="flow-chat-tool-card"
          data-openbitfun-part="statusLayer"
        >
          <StatusGlyph size={size} status={status} />
        </span>
      )}
      {hasToolIcon && (
        <span
          aria-hidden="true"
          className={styles.iconLayer}
          data-openbitfun-component="flow-chat-tool-card"
          data-openbitfun-part="toolIconLayer"
        >
          {toolIcon}
        </span>
      )}
    </span>
  );
}
