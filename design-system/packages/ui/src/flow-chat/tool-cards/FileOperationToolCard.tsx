import type {
  HTMLAttributes,
  MouseEvent as ReactMouseEvent,
  ReactNode,
} from "react";
import {
  ArrowUpRight,
  FileEdit,
  FilePlus,
  FileX2,
  Info,
  TriangleAlert,
  XCircle,
} from "lucide-react";
import { IconButton } from "../../components/IconButton/IconButton";
import { classNames } from "../../internal/classNames";
import {
  AmbientToolCard,
  AmbientToolCardHeader,
  ProminentToolCard,
  ProminentToolCardSummary,
  ToolCardChangeSummary,
  ToolCardActions,
  type FlowChatToolStatus,
} from "./FlowChatToolCard";
import { ToolCardStatusSlot } from "./ToolCardStatusSlot";
import { ToolProcessingDots } from "./ToolProcessingDots";
import styles from "./FileOperationToolCard.module.css";

export type FileOperationKind = "delete" | "edit" | "write";

export interface FileOperationToolCardAction {
  label: string;
  onPress: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  testId?: string;
}

export interface FileOperationToolCardError {
  guidance?: boolean;
  message: ReactNode;
  title: ReactNode;
}

export interface FileOperationToolCardProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "onClick"> {
  actionLabel: ReactNode;
  actionTestId?: string;
  changeSummary?: {
    additions: number | string;
    deletions: number | string;
    label: string;
  };
  error?: FileOperationToolCardError;
  inlineMessage?: ReactNode;
  isExpanded?: boolean;
  onOpenFile?: FileOperationToolCardAction;
  onToggle?: () => void;
  operation: FileOperationKind;
  path: string;
  pathLabel: ReactNode;
  pathTestId?: string;
  preview?: ReactNode;
  requiresConfirmation?: boolean;
  status: FlowChatToolStatus;
  statusDetail?: ReactNode;
}

const OPERATION_ICONS = {
  delete: FileX2,
  edit: FileEdit,
  write: FilePlus,
} as const;

const ACTIVE_STATUSES = new Set<FlowChatToolStatus>([
  "preparing",
  "receiving",
  "running",
  "streaming",
]);

export function FileOperationToolCard({
  actionLabel,
  actionTestId,
  changeSummary,
  className,
  error,
  inlineMessage,
  isExpanded = false,
  onOpenFile,
  onToggle,
  operation,
  path,
  pathLabel,
  pathTestId,
  preview,
  requiresConfirmation = false,
  status,
  statusDetail,
  ...props
}: FileOperationToolCardProps) {
  const Icon = OPERATION_ICONS[operation];
  const failed = status === "error";
  const loading = ACTIVE_STATUSES.has(status);

  if (operation === "delete") {
    return (
      <div
        {...props}
        className={classNames(styles.root, className)}
        data-openbitfun-component="file-operation-tool-card"
        data-openbitfun-operation={operation}
        data-openbitfun-part="root"
        data-openbitfun-status={status}
      >
        <AmbientToolCard
          header={(
            <AmbientToolCardHeader
              action={actionLabel}
              content={(
                <span
                  className={styles.path}
                  data-path={path}
                  data-openbitfun-operation={operation}
                  data-openbitfun-part="path"
                  data-testid={pathTestId}
                  title={path}
                >
                  {pathLabel}
                </span>
              )}
              icon={(
                <ToolCardStatusSlot
                  status={status}
                  toolIcon={<Icon aria-hidden="true" />}
                />
              )}
            />
          )}
          isExpanded={false}
          status={status}
        />
      </div>
    );
  }

  const errorContent = error ? (
    <div className={styles.error} data-guidance={error.guidance ? "true" : "false"}>
      <div className={styles.errorTitle}>
        {error.guidance ? <Info aria-hidden="true" /> : <XCircle aria-hidden="true" />}
        <span>{error.title}</span>
      </div>
      <div className={styles.errorMessage}>{error.message}</div>
    </div>
  ) : undefined;
  const hasPreview = Boolean(preview);
  const hasExpandedContent = failed ? Boolean(errorContent) : hasPreview;

  return (
    <div
      {...props}
      className={classNames(styles.root, className)}
      data-openbitfun-component="file-operation-tool-card"
      data-openbitfun-operation={operation}
      data-openbitfun-part="root"
      data-openbitfun-status={status}
    >
      <ProminentToolCard
        collapsibleErrorContent
        errorContent={errorContent}
        expandedContent={hasPreview ? <div className={styles.preview}>{preview}</div> : undefined}
        summary={(
          <ProminentToolCardSummary
            action={actionLabel}
            actionTestId={actionTestId}
            trailingActions={onOpenFile ? (
              <ToolCardActions>
                <IconButton
                  aria-label={onOpenFile.label}
                  data-openbitfun-affordance="open-panel-right"
                  data-openbitfun-part="openPanelButton"
                  icon={<ArrowUpRight aria-hidden="true" data-openbitfun-icon="open-panel-right" />}
                  onClick={onOpenFile.onPress}
                  size="sm"
                  data-testid={onOpenFile.testId}
                  title={onOpenFile.label}
                  variant="quiet"
                />
              </ToolCardActions>
            ) : undefined}
            content={inlineMessage ? (
              <span className={styles.inlineMessage}>{inlineMessage}</span>
            ) : (
              <span
                className={styles.path}
                data-openbitfun-operation={operation}
                data-path={path}
                data-testid={pathTestId}
                title={path}
              >
                {pathLabel}
              </span>
            )}
            extra={statusDetail ? (
              <span className={styles.statusDetail}>{statusDetail}</span>
            ) : changeSummary ? (
              <ToolCardChangeSummary
                additions={changeSummary.additions}
                aria-label={changeSummary.label}
                deletions={changeSummary.deletions}
              />
            ) : undefined}
            icon={<Icon aria-hidden="true" />}
            statusIcon={failed
              ? (
                <TriangleAlert
                  aria-hidden="true"
                  className={styles.warningStatusIcon}
                  data-openbitfun-icon="warning"
                />
              )
              : loading
                ? <ToolProcessingDots size={16} />
                : undefined}
          />
        )}
        summaryExpandAffordance={hasExpandedContent}
        isExpanded={Boolean(isExpanded && hasExpandedContent)}
        isFailed={failed}
        onToggle={hasExpandedContent && onToggle ? () => onToggle() : undefined}
        requiresConfirmation={requiresConfirmation}
        status={status}
      />
    </div>
  );
}
