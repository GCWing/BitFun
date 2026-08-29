import type { HTMLAttributes, ReactNode } from "react";
import { FileText } from "lucide-react";
import {
  AmbientToolCard,
  AmbientToolCardHeader,
  type FlowChatToolStatus,
} from "./FlowChatToolCard";
import { ToolCardStatusSlot } from "./ToolCardStatusSlot";

export interface ReadFileToolCardProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "onClick"> {
  accessibleLabel?: string;
  interactive?: boolean;
  onOpen?: () => void;
  status: FlowChatToolStatus;
  summary: ReactNode;
}

export function ReadFileToolCard({
  accessibleLabel,
  className,
  interactive = false,
  onOpen,
  status,
  summary,
  ...props
}: ReadFileToolCardProps) {
  const canOpen = interactive && Boolean(onOpen);

  return (
    <AmbientToolCard
      {...props}
      aria-label={accessibleLabel}
      className={className}
      data-bf-tool-card="read-file"
      header={(
        <AmbientToolCardHeader
          content={summary}
          icon={(
            <ToolCardStatusSlot
              status={status}
              toolIcon={<FileText aria-hidden="true" />}
            />
          )}
        />
      )}
      isExpanded={false}
      onClick={canOpen ? () => onOpen?.() : undefined}
      status={status}
    />
  );
}
