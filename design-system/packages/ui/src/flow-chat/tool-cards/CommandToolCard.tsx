import type {
  HTMLAttributes,
  MouseEvent as ReactMouseEvent,
  ReactNode,
} from "react";
import {
  Check,
  Copy,
  ExternalLink,
  Square,
  Terminal,
} from "lucide-react";
import { IconButton } from "../../components/IconButton/IconButton";
import { classNames } from "../../internal/classNames";
import {
  ProminentToolCard,
  ProminentToolCardHeader,
  ToolCardHeaderActions,
  type FlowChatToolStatus,
} from "./FlowChatToolCard";
import { ToolProcessingDots } from "./ToolProcessingDots";
import styles from "./CommandToolCard.module.css";

export interface CommandToolCardAction {
  disabled?: boolean;
  label: string;
  onPress: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  testId?: string;
}

export interface CommandToolCardCopyAction extends CommandToolCardAction {
  copied?: boolean;
  copiedLabel?: string;
}

export interface CommandToolCardFooterItem {
  grow?: boolean;
  label?: ReactNode;
  monospace?: boolean;
  pushToEnd?: boolean;
  tone?: "danger" | "neutral" | "success" | "warning";
  value: ReactNode;
}

export interface CommandToolCardProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "onClick"> {
  action: ReactNode;
  command?: string | null;
  commandTestId?: string;
  copyAction?: CommandToolCardCopyAction;
  emptyCommand: ReactNode;
  error?: ReactNode;
  footerItems?: readonly CommandToolCardFooterItem[];
  interruptAction?: CommandToolCardAction;
  isExpanded: boolean;
  onToggle?: () => void;
  openAction?: CommandToolCardAction;
  output?: ReactNode;
  outputAction?: ReactNode;
  outputDensity?: "compact" | "expanded";
  outputSizing?: "content" | "fixed";
  reserveFooter?: boolean;
  reserveOutput?: boolean;
  requiresConfirmation?: boolean;
  status: FlowChatToolStatus;
  statusLabel?: ReactNode;
  statusSummary?: ReactNode;
  statusTone?: "danger" | "neutral" | "warning";
  toggleTestId?: string;
  waitingContent?: ReactNode;
}

const ACTIVE_STATUSES = new Set<FlowChatToolStatus>([
  "preparing",
  "receiving",
  "running",
  "streaming",
]);

export function CommandToolCard({
  action,
  className,
  command,
  commandTestId,
  copyAction,
  emptyCommand,
  error,
  footerItems = [],
  interruptAction,
  isExpanded,
  onToggle,
  openAction,
  output,
  outputAction,
  outputDensity = "expanded",
  outputSizing = "fixed",
  reserveFooter = false,
  reserveOutput = false,
  requiresConfirmation = false,
  status,
  statusLabel,
  statusSummary,
  statusTone = "neutral",
  toggleTestId,
  waitingContent,
  ...props
}: CommandToolCardProps) {
  const loading = ACTIVE_STATUSES.has(status);
  const failed = status === "error";
  const hasOutputFrame = Boolean(output || waitingContent || reserveOutput);
  const hasFooter = reserveFooter || footerItems.length > 0;
  const hasDetails = hasOutputFrame || hasFooter;
  const resolvedCommand = command?.trim() ? command : null;
  const actionItems = Boolean(copyAction || openAction || interruptAction);

  const renderAction = (
    kind: "copy" | "interrupt" | "open",
    item: CommandToolCardAction | CommandToolCardCopyAction | undefined,
  ) => {
    if (!item) return null;
    const copied = kind === "copy" && "copied" in item && item.copied;
    const label = copied && "copiedLabel" in item && item.copiedLabel
      ? item.copiedLabel
      : item.label;
    const icon = kind === "copy"
      ? copied ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />
      : kind === "open"
        ? <ExternalLink aria-hidden="true" />
        : <Square aria-hidden="true" fill="currentColor" />;

    return (
      <IconButton
        aria-label={label}
        className={kind === "interrupt" ? styles.criticalAction : undefined}
        disabled={item.disabled}
        icon={icon}
        onClick={item.onPress}
        size="sm"
        data-testid={item.testId}
        title={label}
        tone={kind === "interrupt" ? "danger" : "neutral"}
        variant="quiet"
      />
    );
  };

  const details = hasDetails ? (
    <div className={styles.details} data-bf-part="details">
      {hasOutputFrame && (
        <div
          className={styles.outputFrame}
          data-bf-part="outputFrame"
          data-density={outputDensity}
          data-sizing={outputSizing}
        >
          {outputAction && <span className={styles.outputActions}>{outputAction}</span>}
          {output
            ? <div className={styles.output} data-bf-part="output">{output}</div>
            : <div className={styles.waiting} data-bf-part="waiting">{waitingContent}</div>}
        </div>
      )}
      {hasFooter && (
        <div className={styles.footer} data-bf-part="footer">
          {footerItems.map((item, index) => (
            <span
              className={styles.footerItem}
              data-grow={item.grow ? "true" : "false"}
              data-monospace={item.monospace ? "true" : "false"}
              data-push-to-end={item.pushToEnd ? "true" : "false"}
              data-tone={item.tone ?? "neutral"}
              key={index}
            >
              {item.label !== undefined && item.label !== null && (
                <span className={styles.footerLabel}>{item.label}</span>
              )}
              <span className={styles.footerValue}>{item.value}</span>
            </span>
          ))}
        </div>
      )}
    </div>
  ) : undefined;

  return (
    <div
      {...props}
      className={classNames(styles.root, className)}
      data-bf-component="command-tool-card"
      data-bf-part="root"
      data-bf-status={status}
    >
      <ProminentToolCard
        errorContent={error ? <div className={styles.error}>{error}</div> : undefined}
        expandedContent={details}
        header={(
          <ProminentToolCardHeader
            action={action}
            actions={actionItems ? (
              <ToolCardHeaderActions>
                {renderAction("interrupt", interruptAction)}
                {renderAction("copy", copyAction)}
                {renderAction("open", openAction)}
              </ToolCardHeaderActions>
            ) : undefined}
            content={(
              <code
                className={styles.command}
                data-bf-part="command"
                data-empty={resolvedCommand ? "false" : "true"}
                data-testid={commandTestId}
                title={resolvedCommand ?? undefined}
              >
                {resolvedCommand ?? emptyCommand}
              </code>
            )}
            extra={(statusSummary || statusLabel) ? (
              <span className={styles.statusSummary} data-bf-part="statusSummary">
                {statusSummary}
                {statusLabel && (
                  <span className={styles.statusLabel} data-tone={statusTone}>{statusLabel}</span>
                )}
              </span>
            ) : undefined}
            icon={<Terminal aria-hidden="true" />}
            statusIcon={loading ? <ToolProcessingDots size={16} /> : undefined}
          />
        )}
        headerExpandAffordance={hasDetails}
        isExpanded={isExpanded}
        isFailed={failed}
        onClick={hasDetails && onToggle ? () => onToggle() : undefined}
        requiresConfirmation={requiresConfirmation}
        status={status}
        toggleTestId={toggleTestId}
      />
    </div>
  );
}
