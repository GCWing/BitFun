import type { HTMLAttributes, ReactNode } from "react";
import { FileText } from "lucide-react";
import {
  AmbientToolCard,
  AmbientToolCardHeader,
  type FlowChatToolStatus,
} from "./FlowChatToolCard";
import { ToolCardStatusSlot } from "./ToolCardStatusSlot";

export interface ReadFileToolCardProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "content" | "onClick"> {
  accessibleLabel?: string;
  action?: ReactNode;
  content?: ReactNode;
  interactive?: boolean;
  onOpen?: () => void;
  status: FlowChatToolStatus;
}

export function ReadFileToolCard({
  accessibleLabel,
  action,
  className,
  content,
  interactive = false,
  onOpen,
  status,
  ...props
}: ReadFileToolCardProps) {
  const canOpen = interactive && Boolean(onOpen);

  return (
    <AmbientToolCard
      {...props}
      aria-label={accessibleLabel}
      className={className}
      data-openbitfun-tool-card="read-file"
      header={(
        <AmbientToolCardHeader
          action={action}
          content={content}
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
