import type { HTMLAttributes, ReactNode } from "react";
import { Layers, MessageSquare } from "lucide-react";
import {
  AmbientToolCard,
  AmbientToolCardHeader,
  type FlowChatToolStatus,
} from "./FlowChatToolCard";
import { ToolCardStatusSlot } from "./ToolCardStatusSlot";
import styles from "./SessionToolCards.module.css";

export interface SessionToolCardField {
  label: ReactNode;
  monospace?: boolean;
  value: ReactNode;
}

export interface SessionToolCardSession {
  agentType?: ReactNode;
  id: ReactNode;
  key: string;
  name?: ReactNode;
}

interface SessionToolCardBaseProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "onClick"> {
  action: ReactNode;
  emptyState?: ReactNode;
  error?: ReactNode;
  fields?: readonly SessionToolCardField[];
  icon: ReactNode;
  isExpanded?: boolean;
  message?: ReactNode;
  messageLabel?: ReactNode;
  onToggle?: () => void;
  sessions?: readonly SessionToolCardSession[];
  status: FlowChatToolStatus;
  summary: ReactNode;
  toolCard: string;
}

function SessionToolCardBase({
  action,
  emptyState,
  error,
  fields = [],
  icon,
  isExpanded = false,
  message,
  messageLabel,
  onToggle,
  sessions = [],
  status,
  summary,
  toolCard,
  ...props
}: SessionToolCardBaseProps) {
  const hasDetails = fields.length > 0 || sessions.length > 0 || Boolean(emptyState || message || error);
  const expandedContent = hasDetails ? (
    <div className={styles.details} data-bf-part="details">
      {fields.map((field, index) => (
        <div
          className={styles.field}
          data-bf-part="field"
          data-monospace={field.monospace ? "true" : "false"}
          key={index}
        >
          <span className={styles.fieldLabel}>{field.label}</span>
          <span className={styles.fieldValue}>{field.value}</span>
        </div>
      ))}

      {sessions.length > 0 && (
        <div className={styles.sessionList} data-bf-part="sessionList">
          {sessions.map((session) => (
            <div className={styles.session} data-bf-part="session" key={session.key}>
              <span className={styles.sessionId}>{session.id}</span>
              {session.name && <span className={styles.sessionName}>{session.name}</span>}
              {session.agentType && <span className={styles.sessionAgent}>{session.agentType}</span>}
            </div>
          ))}
        </div>
      )}

      {message && (
        <div className={styles.messageSection} data-bf-part="messageSection">
          {messageLabel && <span className={styles.sectionLabel}>{messageLabel}</span>}
          <pre className={styles.message} data-bf-part="message">{message}</pre>
        </div>
      )}

      {emptyState && <div className={styles.empty} data-bf-part="empty">{emptyState}</div>}
      {error && <div className={styles.error} data-bf-part="error">{error}</div>}
    </div>
  ) : undefined;

  return (
    <AmbientToolCard
      {...props}
      data-bf-tool-card={toolCard}
      expandedContent={expandedContent}
      header={(
        <AmbientToolCardHeader
          action={action}
          content={summary}
          icon={<ToolCardStatusSlot status={status} toolIcon={icon} />}
        />
      )}
      isExpanded={Boolean(isExpanded && hasDetails)}
      onClick={hasDetails && onToggle ? onToggle : undefined}
      status={status}
    />
  );
}

export interface SessionControlToolCardProps
  extends Omit<SessionToolCardBaseProps, "icon" | "message" | "messageLabel" | "toolCard"> {}

export function SessionControlToolCard(props: SessionControlToolCardProps) {
  return <SessionToolCardBase {...props} icon={<Layers aria-hidden="true" />} toolCard="session-control" />;
}

export interface SessionMessageToolCardProps
  extends Omit<SessionToolCardBaseProps, "emptyState" | "icon" | "sessions" | "toolCard"> {}

export function SessionMessageToolCard(props: SessionMessageToolCardProps) {
  return <SessionToolCardBase {...props} icon={<MessageSquare aria-hidden="true" />} toolCard="session-message" />;
}
