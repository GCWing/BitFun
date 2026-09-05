import type { HTMLAttributes, ReactNode } from "react";
import { Hourglass, Info, Terminal, Zap } from "lucide-react";
import {
  AmbientToolCard,
  AmbientToolCardHeader,
  type FlowChatToolStatus,
} from "./FlowChatToolCard";
import {
  ToolCardStatusSlot,
  type ToolCardStatusSlotProps,
} from "./ToolCardStatusSlot";
import styles from "./ActivityToolCards.module.css";

interface ActivityToolCardBaseProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "onClick"> {
  action?: ReactNode;
  defaultIcon?: ToolCardStatusSlotProps["defaultIcon"];
  icon: ReactNode;
  status: FlowChatToolStatus;
  summary?: ReactNode;
  summaryTitle?: string;
  toolCard: string;
}

function ActivityToolCardBase({
  action,
  defaultIcon,
  icon,
  status,
  summary,
  summaryTitle,
  toolCard,
  ...props
}: ActivityToolCardBaseProps) {
  return (
    <AmbientToolCard
      {...props}
      data-openbitfun-tool-card={toolCard}
      header={(
        <AmbientToolCardHeader
          action={action}
          content={summary !== undefined && summary !== null ? (
            <span
              className={styles.summary}
              data-openbitfun-part="summary"
              data-tone={status === "error" ? "danger" : "neutral"}
              title={summaryTitle}
            >
              {summary}
            </span>
          ) : undefined}
          icon={(
            <ToolCardStatusSlot
              defaultIcon={defaultIcon}
              status={status}
              toolIcon={icon}
            />
          )}
        />
      )}
      status={status}
    />
  );
}

export type ActivityToolCardProps = Omit<
  ActivityToolCardBaseProps,
  "icon" | "toolCard"
>;

export function AgentWaitToolCard(props: ActivityToolCardProps) {
  return (
    <ActivityToolCardBase
      {...props}
      icon={<Hourglass aria-hidden="true" />}
      toolCard="agent-wait"
    />
  );
}

export function GetToolSpecToolCard(props: ActivityToolCardProps) {
  return (
    <ActivityToolCardBase
      {...props}
      icon={<Info aria-hidden="true" />}
      toolCard="get-tool-spec"
    />
  );
}

export function SkillToolCard(props: ActivityToolCardProps) {
  return (
    <ActivityToolCardBase
      {...props}
      icon={<Zap aria-hidden="true" />}
      toolCard="skill"
    />
  );
}

export function TerminalControlToolCard(props: ActivityToolCardProps) {
  return (
    <ActivityToolCardBase
      {...props}
      icon={<Terminal aria-hidden="true" />}
      toolCard="terminal-control"
    />
  );
}
