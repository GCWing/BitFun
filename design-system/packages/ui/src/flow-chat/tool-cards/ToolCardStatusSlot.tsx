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
    case "cancelled":
    case "rejected":
      return <X {...iconProps} className={styles.muted} />;
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
  return (
    <span
      className={classNames(styles.root, className)}
      data-bf-component="flow-chat-tool-card"
      data-bf-part="statusSlot"
      data-default-icon={defaultIcon}
    >
      <span
        className={styles.statusLayer}
        data-bf-component="flow-chat-tool-card"
        data-bf-part="statusLayer"
      >
        <StatusGlyph size={size} status={status} />
      </span>
      {toolIcon !== undefined && toolIcon !== null && toolIcon !== false && (
        <span
          aria-hidden="true"
          className={styles.iconLayer}
          data-bf-component="flow-chat-tool-card"
          data-bf-part="toolIconLayer"
        >
          {toolIcon}
        </span>
      )}
    </span>
  );
}
