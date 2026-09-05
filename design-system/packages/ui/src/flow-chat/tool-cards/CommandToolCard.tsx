import type {
  HTMLAttributes,
  MouseEvent as ReactMouseEvent,
  ReactNode,
} from "react";
import {
  ExternalLink,
  Square,
  Terminal,
} from "lucide-react";
import { IconButton } from "../../components/IconButton/IconButton";
import { classNames } from "../../internal/classNames";
import {
  ProminentToolCard,
  ProminentToolCardSummary,
  ToolCardActions,
  type FlowChatToolStatus,
} from "./FlowChatToolCard";
import { ToolCardCopyButton } from "./ToolCardCopyButton";
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
    if (kind === "copy") {
      const copyItem = item as CommandToolCardCopyAction;
      return (
        <ToolCardCopyButton
          copied={copied}
          copiedLabel={copyItem.copiedLabel}
          disabled={copyItem.disabled}
          label={copyItem.label}
          onPress={copyItem.onPress}
          testId={copyItem.testId}
        />
      );
    }

    const icon = kind === "open"
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
    <div className={styles.details} data-openbitfun-part="details">
      {hasOutputFrame && (
        <div
          className={styles.outputFrame}
          data-openbitfun-part="outputFrame"
          data-density={outputDensity}
          data-sizing={outputSizing}
        >
          {outputAction && <span className={styles.outputActions}>{outputAction}</span>}
          {output
            ? <div className={styles.output} data-openbitfun-part="output">{output}</div>
            : <div className={styles.waiting} data-openbitfun-part="waiting">{waitingContent}</div>}
        </div>
      )}
      {hasFooter && (
        <div className={styles.footer} data-openbitfun-part="footer">
          {footerItems.map((item, index) => (
            <span
              className={styles.footerItem}
              data-grow={item.grow ? "true" : "false"}
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
      data-openbitfun-component="command-tool-card"
      data-openbitfun-part="root"
      data-openbitfun-status={status}
    >
      <ProminentToolCard
        errorContent={error ? <div className={styles.error}>{error}</div> : undefined}
        expandedContent={details}
        summary={(
          <ProminentToolCardSummary
            action={action}
            actions={actionItems ? (
              <ToolCardActions>
                {renderAction("interrupt", interruptAction)}
                {renderAction("copy", copyAction)}
                {renderAction("open", openAction)}
              </ToolCardActions>
            ) : undefined}
            content={(
              <code
                className={styles.command}
                data-openbitfun-part="command"
                data-empty={resolvedCommand ? "false" : "true"}
                data-testid={commandTestId}
                title={resolvedCommand ?? undefined}
              >
                {resolvedCommand ?? emptyCommand}
              </code>
            )}
            extra={(statusSummary || statusLabel) ? (
              <span className={styles.statusSummary} data-openbitfun-part="statusSummary">
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
        summaryExpandAffordance={hasDetails}
        isExpanded={isExpanded}
        isFailed={failed}
        onToggle={hasDetails && onToggle ? () => onToggle() : undefined}
        requiresConfirmation={requiresConfirmation}
        status={status}
        toggleTestId={toggleTestId}
      />
    </div>
  );
}
