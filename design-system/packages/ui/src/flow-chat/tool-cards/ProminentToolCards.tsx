import type {
  HTMLAttributes,
  MouseEvent as ReactMouseEvent,
  ReactNode,
} from "react";
import {
  Bot,
  ChevronDown,
  ChevronRight,
  FileText,
  GitBranch,
  GitCompare,
  Rocket,
  SearchCheck,
  Sparkles,
} from "lucide-react";
import { classNames } from "../../internal/classNames";
import {
  ProminentToolCard,
  ProminentToolCardHeader,
  ToolCardChangeSummary,
  ToolCardHeaderActions,
  type FlowChatToolStatus,
} from "./FlowChatToolCard";
import { ToolProcessingDots } from "./ToolProcessingDots";
import styles from "./ProminentToolCards.module.css";

interface ProminentCardProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "onClick" | "title"> {
  isExpanded?: boolean;
  onToggle?: () => void;
  status: FlowChatToolStatus;
}

export interface GitToolCardFooterItem {
  grow?: boolean;
  label?: ReactNode;
  monospace?: boolean;
  tone?: "danger" | "neutral" | "success";
  value: ReactNode;
}

export interface GitToolCardProps extends ProminentCardProps {
  action: ReactNode;
  command: ReactNode;
  error?: ReactNode;
  errorMeta?: ReactNode;
  footerItems?: readonly GitToolCardFooterItem[];
  headerActions?: ReactNode;
  loading?: boolean;
  statusSummary?: ReactNode;
  statusTone?: "danger" | "neutral" | "warning";
  stderr?: ReactNode;
  stderrLabel?: ReactNode;
  stderrTone?: "danger" | "warning";
  stdout?: ReactNode;
}

export function GitToolCard({
  action,
  command,
  error,
  errorMeta,
  footerItems = [],
  headerActions,
  isExpanded = false,
  loading = false,
  onToggle,
  status,
  statusSummary,
  statusTone = "neutral",
  stderr,
  stderrLabel,
  stderrTone = "danger",
  stdout,
  ...props
}: GitToolCardProps) {
  const hasDetails = Boolean(stdout || stderr || error || footerItems.length > 0);
  const body = hasDetails ? (
    <div className={styles.output} data-bf-part="details">
      {stdout && <pre className={styles.outputBlock} data-bf-part="stdout">{stdout}</pre>}
      {stderr && (
        <div className={styles.outputGroup} data-bf-part="stderr" data-tone={stderrTone}>
          {stderrLabel && <span className={styles.outputLabel}>{stderrLabel}</span>}
          <pre className={styles.outputBlock}>{stderr}</pre>
        </div>
      )}
      {error && (
        <div className={styles.error} data-bf-part="error">
          {error}
          {errorMeta && <div className={styles.errorMeta}>{errorMeta}</div>}
        </div>
      )}
      {footerItems.length > 0 && (
        <div className={styles.footer} data-bf-part="footer">
          {footerItems.map((item, index) => (
            <span
              className={styles.footerItem}
              data-grow={item.grow ? "true" : "false"}
              data-monospace={item.monospace ? "true" : "false"}
              data-tone={item.tone ?? "neutral"}
              key={index}
            >
              {item.label && <span className={styles.footerLabel}>{item.label}</span>}
              <span className={styles.footerValue}>{item.value}</span>
            </span>
          ))}
        </div>
      )}
    </div>
  ) : undefined;

  return (
    <ProminentToolCard
      {...props}
      data-bf-tool-card="git"
      errorContent={status === "error" ? body : undefined}
      expandedContent={status === "error" ? undefined : body}
      header={(
        <ProminentToolCardHeader
          action={action}
          actions={headerActions ? <ToolCardHeaderActions>{headerActions}</ToolCardHeaderActions> : undefined}
          content={<code className={styles.command}>{command}</code>}
          extra={statusSummary ? (
            <span className={styles.summary} data-tone={statusTone}>{statusSummary}</span>
          ) : undefined}
          icon={<GitBranch aria-hidden="true" />}
          statusIcon={loading ? <ToolProcessingDots size={16} /> : undefined}
        />
      )}
      headerExpandAffordance={hasDetails}
      isExpanded={Boolean(isExpanded && hasDetails && status !== "error")}
      onClick={hasDetails && onToggle ? onToggle : undefined}
      status={status}
    />
  );
}

export interface FileDiffToolCardProps extends ProminentCardProps {
  action: ReactNode;
  changeSummary?: {
    additions: number | string;
    deletions: number | string;
    label: string;
  };
  error?: ReactNode;
  loading?: boolean;
  message?: ReactNode;
  path: string;
  pathLabel: ReactNode;
  preview?: ReactNode;
  textPreview?: ReactNode;
}

export function FileDiffToolCard({
  action,
  changeSummary,
  error,
  isExpanded = false,
  loading = false,
  message,
  onToggle,
  path,
  pathLabel,
  preview,
  status,
  textPreview,
  ...props
}: FileDiffToolCardProps) {
  const body = preview || textPreview || message ? (
    <div className={styles.diffBody} data-bf-part="details">
      {message && <div className={styles.message}>{message}</div>}
      {preview}
      {textPreview && <pre className={styles.textPreview}>{textPreview}</pre>}
    </div>
  ) : undefined;
  return (
    <ProminentToolCard
      {...props}
      data-bf-tool-card="file-diff"
      errorContent={error ? <div className={styles.error}>{error}</div> : undefined}
      expandedContent={body}
      header={(
        <ProminentToolCardHeader
          action={action}
          content={(
            <span
              className={styles.diffPath}
              data-path={path}
              data-bf-part="path"
              title={path}
            >
              {pathLabel}
            </span>
          )}
          extra={changeSummary ? (
            <ToolCardChangeSummary
              additions={changeSummary.additions}
              aria-label={changeSummary.label}
              deletions={changeSummary.deletions}
            />
          ) : undefined}
          icon={<GitCompare aria-hidden="true" />}
          statusIcon={loading ? <ToolProcessingDots size={16} /> : undefined}
        />
      )}
      headerExpandAffordance={Boolean(body)}
      isExpanded={Boolean(isExpanded && body && status !== "error")}
      onClick={body && onToggle ? onToggle : undefined}
      status={status}
    />
  );
}

export interface ReviewSummaryToolCardProps extends ProminentCardProps {
  action?: ReactNode;
  changedFiles?: readonly string[];
  fileCountLabel?: ReactNode;
  filesLabel?: ReactNode;
  kind?: "deep-review" | "review";
  loading?: boolean;
  summary: ReactNode;
  title: ReactNode;
}

export function ReviewSummaryToolCard({
  action,
  changedFiles = [],
  fileCountLabel,
  filesLabel,
  isExpanded = false,
  kind = "review",
  loading = false,
  onToggle,
  status,
  summary,
  title,
  ...props
}: ReviewSummaryToolCardProps) {
  const Icon = kind === "deep-review" ? Sparkles : SearchCheck;
  return (
    <ProminentToolCard
      {...props}
      data-bf-tool-card="review-summary"
      expandedContent={(
        <div className={styles.reviewDetails} data-bf-part="details">
          <p className={styles.reviewSummary}>{summary}</p>
          {changedFiles.length > 0 && (
            <div>
              {filesLabel && <div className={styles.sectionLabel}>{filesLabel}</div>}
              <ul className={styles.fileList}>
                {changedFiles.map((file) => <li key={file}>{file}</li>)}
              </ul>
            </div>
          )}
          {action && <div className={styles.actions}>{action}</div>}
        </div>
      )}
      header={(
        <ProminentToolCardHeader
          content={title}
          extra={changedFiles.length > 0 && fileCountLabel ? (
            <span className={styles.fileCount}><FileText aria-hidden="true" />{fileCountLabel}</span>
          ) : undefined}
          icon={<Icon aria-hidden="true" />}
          statusIcon={loading ? <ToolProcessingDots size={16} /> : undefined}
        />
      )}
      isExpanded={isExpanded}
      onClick={onToggle}
      status={status}
    />
  );
}

export interface PageLifecycleToolCardField {
  label: ReactNode;
  monospace?: boolean;
  value: ReactNode;
}

interface PageLifecycleToolCardBaseProps extends ProminentCardProps {
  action: ReactNode;
  actions?: ReactNode;
  error?: ReactNode;
  fields?: readonly PageLifecycleToolCardField[];
  loading?: boolean;
  subject: ReactNode;
  toolCard: "page-deploy" | "page-publish";
  version?: ReactNode;
}

function PageLifecycleToolCardBase({
  action,
  actions,
  error,
  fields = [],
  isExpanded = false,
  loading = false,
  onToggle,
  status,
  subject,
  toolCard,
  version,
  ...props
}: PageLifecycleToolCardBaseProps) {
  const hasDetails = fields.length > 0 || Boolean(actions || error);
  const body = hasDetails ? (
    <div className={styles.lifecycleDetails} data-bf-part="details">
      {fields.map((field, index) => (
        <div className={styles.field} data-monospace={field.monospace ? "true" : "false"} key={index}>
          <span className={styles.fieldLabel}>{field.label}</span>
          <span className={styles.fieldValue}>{field.value}</span>
        </div>
      ))}
      {error && <div className={styles.error}>{error}</div>}
      {actions && <div className={styles.actions}>{actions}</div>}
    </div>
  ) : undefined;
  return (
    <ProminentToolCard
      {...props}
      data-bf-tool-card={toolCard}
      errorContent={status === "error" ? body : undefined}
      expandedContent={status === "error" ? undefined : body}
      header={(
        <ProminentToolCardHeader
          action={action}
          content={<span className={styles.command}>{subject}{version ? ` @ ${version}` : ""}</span>}
          icon={<Rocket aria-hidden="true" />}
          statusIcon={loading ? <ToolProcessingDots size={16} /> : undefined}
        />
      )}
      headerExpandAffordance={hasDetails}
      isExpanded={Boolean(isExpanded && hasDetails && status !== "error")}
      onClick={hasDetails && onToggle ? onToggle : undefined}
      status={status}
    />
  );
}

export type PageDeployToolCardProps = Omit<PageLifecycleToolCardBaseProps, "toolCard">;
export function PageDeployToolCard(props: PageDeployToolCardProps) {
  return <PageLifecycleToolCardBase {...props} toolCard="page-deploy" />;
}

export type PagePublishToolCardProps = Omit<PageLifecycleToolCardBaseProps, "toolCard">;
export function PagePublishToolCard(props: PagePublishToolCardProps) {
  return <PageLifecycleToolCardBase {...props} toolCard="page-publish" />;
}

export interface AgentControlToolCardProps extends ProminentCardProps {
  agentName: ReactNode;
  avatar?: ReactNode;
  onOpenAgent?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  openAgentLabel?: string;
  prompt?: ReactNode;
  statusLabel: ReactNode;
  statusTone?: "danger" | "neutral" | "success" | "warning";
}

export function AgentControlToolCard({
  agentName,
  avatar,
  className,
  isExpanded = false,
  onOpenAgent,
  onToggle,
  openAgentLabel,
  prompt,
  status,
  statusLabel,
  statusTone = "neutral",
  ...props
}: AgentControlToolCardProps) {
  const expandable = Boolean(prompt && onToggle);
  const agentPillContent = (
    <>
      <span className={styles.agentAvatar} data-bf-part="avatar">
        {avatar ?? <Bot aria-hidden="true" />}
      </span>
      <span className={styles.agentName} data-bf-part="agentName">{agentName}</span>
    </>
  );
  const header = (
    <div className={styles.agentHeader} data-bf-part="agentHeader">
      {onOpenAgent ? (
        <button
          aria-label={openAgentLabel}
          className={styles.agentPill}
          data-bf-part="agentIdentity"
          onClick={onOpenAgent}
          title={openAgentLabel}
          type="button"
        >
          {agentPillContent}
        </button>
      ) : <span className={styles.agentPill} data-bf-part="agentIdentity">{agentPillContent}</span>}
      <span className={styles.agentStatus} data-bf-part="agentStatus" data-tone={statusTone}>
        {statusLabel}
      </span>
      {expandable && (
        <span
          aria-hidden="true"
          className={styles.agentIndicator}
          data-bf-part="expandIndicator"
        >
          {isExpanded ? <ChevronDown /> : <ChevronRight />}
        </span>
      )}
    </div>
  );

  return (
    <ProminentToolCard
      {...props}
      className={classNames(styles.agentRoot, className)}
      data-bf-tool-card="agent-control"
      expandedContent={prompt ? <div className={styles.agentPrompt}>{prompt}</div> : undefined}
      header={header}
      headerExpandAffordance={false}
      isExpanded={Boolean(isExpanded && prompt)}
      onClick={expandable ? onToggle : undefined}
      status={status}
    />
  );
}
