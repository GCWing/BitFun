import type {
  HTMLAttributes,
  MouseEvent as ReactMouseEvent,
  ReactNode,
} from "react";
import {
  CheckCircle2,
  Circle,
  Code2,
  Globe,
  Image as ImageIcon,
  Link,
  ListTodo,
  XCircle,
} from "lucide-react";
import {
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from "../../components/Dialog";
import {
  AmbientToolCard,
  AmbientToolCardHeader,
  ToolCardHeaderActions,
  type FlowChatToolStatus,
} from "./FlowChatToolCard";
import { ToolCardStatusSlot } from "./ToolCardStatusSlot";
import { ToolProcessingDots } from "./ToolProcessingDots";
import styles from "./StandardAmbientToolCards.module.css";

interface AmbientCardProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "content" | "onClick" | "title"> {
  isExpanded?: boolean;
  onToggle?: () => void;
  status: FlowChatToolStatus;
}

function Section({ children, label }: { children: ReactNode; label?: ReactNode }) {
  return (
    <section className={styles.section} data-bf-part="section">
      {label && <div className={styles.sectionLabel} data-bf-part="sectionLabel">{label}</div>}
      {children}
    </section>
  );
}

export interface RunCodeToolCardProps extends AmbientCardProps {
  action: ReactNode;
  error?: ReactNode;
  headerActions?: ReactNode;
  output?: ReactNode;
  outputLabel?: ReactNode;
  program?: ReactNode;
  programLabel?: ReactNode;
  summary: ReactNode;
}

export function RunCodeToolCard({
  action,
  error,
  headerActions,
  isExpanded = false,
  onToggle,
  output,
  outputLabel,
  program,
  programLabel,
  status,
  summary,
  ...props
}: RunCodeToolCardProps) {
  const hasDetails = Boolean(program || output || error);
  return (
    <AmbientToolCard
      {...props}
      data-bf-tool-card="run-code"
      expandedContent={hasDetails ? (
        <div className={styles.sections} data-bf-part="details">
          {program && <Section label={programLabel}>{program}</Section>}
          {(output || error) && (
            <Section label={outputLabel}>
              {error
                ? <div className={styles.error}>{error}</div>
                : <div className={styles.resultBlock} data-bf-part="output">{output}</div>}
            </Section>
          )}
        </div>
      ) : undefined}
      header={(
        <AmbientToolCardHeader
          action={action}
          content={summary}
          extra={headerActions ? <ToolCardHeaderActions>{headerActions}</ToolCardHeaderActions> : undefined}
          icon={<ToolCardStatusSlot status={status} toolIcon={<Code2 aria-hidden="true" />} />}
        />
      )}
      isExpanded={Boolean(isExpanded && hasDetails)}
      onClick={hasDetails && onToggle ? onToggle : undefined}
      status={status}
    />
  );
}

export interface WebFetchToolCardProps extends AmbientCardProps {
  action?: ReactNode;
  content?: ReactNode;
  copyAction?: ReactNode;
  details?: readonly ReactNode[];
  emptyContent?: ReactNode;
  error?: ReactNode;
  onOpenUrl?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  openUrlLabel?: string;
  title: ReactNode;
  url?: string;
}

export function WebFetchToolCard({
  action,
  content,
  copyAction,
  details = [],
  emptyContent,
  error,
  isExpanded = false,
  onOpenUrl,
  onToggle,
  openUrlLabel,
  status,
  title,
  url,
  ...props
}: WebFetchToolCardProps) {
  const hasDetails = Boolean(url || content || emptyContent || error);
  return (
    <AmbientToolCard
      {...props}
      data-bf-tool-card="web-fetch"
      expandedContent={hasDetails ? (
        <div className={styles.fetchMeta} data-bf-part="details">
          {url && (
            onOpenUrl ? (
              <button
                aria-label={openUrlLabel}
                className={styles.openLink}
                data-bf-part="sourceLink"
                onClick={onOpenUrl}
                title={url}
                type="button"
              >
                <Link aria-hidden="true" />
                <span className={styles.openLinkText}>{url}</span>
              </button>
            ) : <span className={styles.openLinkText}>{url}</span>
          )}
          {(details.length > 0 || copyAction) && (
            <div className={styles.detailsRow} data-bf-part="detailsRow">
              <span className={styles.pills}>
                {details.map((detail, index) => (
                  <span className={styles.pill} data-bf-part="detail" key={index}>{detail}</span>
                ))}
              </span>
              {copyAction && <ToolCardHeaderActions>{copyAction}</ToolCardHeaderActions>}
            </div>
          )}
          {error ? (
            <div className={styles.error} data-bf-part="error">{error}</div>
          ) : (
            <pre className={styles.resultBlock} data-bf-part="content">{content || emptyContent}</pre>
          )}
        </div>
      ) : undefined}
      header={(
        <AmbientToolCardHeader
          action={action}
          content={<span className={styles.fetchTitle} title={typeof title === "string" ? title : undefined}>{title}</span>}
          icon={(
            <ToolCardStatusSlot
              defaultIcon={status === "completed" || status === "error" ? "tool" : "status"}
              status={status}
              toolIcon={<Globe aria-hidden="true" />}
            />
          )}
        />
      )}
      isExpanded={Boolean(isExpanded && hasDetails)}
      onClick={hasDetails && onToggle ? onToggle : undefined}
      status={status}
    />
  );
}

export interface DefaultToolCardProps extends AmbientCardProps {
  description?: ReactNode;
  displayName: ReactNode;
  error?: ReactNode;
  hasDetails?: boolean;
  icon?: ReactNode;
  inputLabel?: ReactNode;
  inputPreview?: ReactNode;
  requiresConfirmation?: boolean;
  resultLabel?: ReactNode;
  resultPreview?: ReactNode;
  summary: ReactNode;
  toolName: ReactNode;
}

export function DefaultToolCard({
  description,
  displayName,
  error,
  hasDetails,
  icon,
  inputLabel,
  inputPreview,
  isExpanded = false,
  onToggle,
  requiresConfirmation = false,
  resultLabel,
  resultPreview,
  status,
  summary,
  toolName,
  ...props
}: DefaultToolCardProps) {
  const detailsAvailable = hasDetails ?? Boolean(inputPreview || resultPreview || error);
  return (
    <AmbientToolCard
      {...props}
      data-bf-confirmation={requiresConfirmation ? "true" : "false"}
      data-bf-tool-card="default"
      expandedContent={detailsAvailable ? (
        <div className={styles.sections} data-bf-part="details">
          <div className={styles.meta} data-bf-part="meta">
            <span className={styles.metaLabel}>{toolName}</span>
            {description && <span className={styles.metaDescription}>{description}</span>}
          </div>
          {inputPreview && (
            <Section label={inputLabel}>
              <pre className={styles.codeBlock} data-bf-part="input">{inputPreview}</pre>
            </Section>
          )}
          {(resultPreview || error) && (
            <Section label={resultLabel}>
              {error
                ? <div className={styles.error} data-bf-part="error">{error}</div>
                : <pre className={styles.codeBlock} data-bf-part="result">{resultPreview}</pre>}
            </Section>
          )}
        </div>
      ) : undefined}
      header={(
        <AmbientToolCardHeader
          action={displayName}
          content={summary}
          icon={<ToolCardStatusSlot status={status} toolIcon={icon} />}
        />
      )}
      isExpanded={Boolean(isExpanded && detailsAvailable)}
      onClick={detailsAvailable && onToggle ? onToggle : undefined}
      status={status}
    />
  );
}

export interface ViewImageToolCardProps extends AmbientCardProps {
  alt: string;
  errorText?: ReactNode;
  height?: number;
  imageFailed?: boolean;
  lightboxOpen?: boolean;
  lightboxTitle?: ReactNode;
  onImageError?: () => void;
  onLightboxClose?: () => void;
  onOpenPreview?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  previewLabel: string;
  source?: string;
  statusText: ReactNode;
  width?: number;
}

export function ViewImageToolCard({
  alt,
  errorText,
  height,
  imageFailed = false,
  isExpanded = false,
  lightboxOpen = false,
  lightboxTitle,
  onImageError,
  onLightboxClose,
  onOpenPreview,
  onToggle,
  previewLabel,
  source,
  status,
  statusText,
  width,
  ...props
}: ViewImageToolCardProps) {
  return (
    <>
      <AmbientToolCard
        {...props}
        data-bf-tool-card="view-image"
        expandedContent={source ? (
          <div className={styles.imageContent} data-bf-part="imageContent">
            {imageFailed ? (
              <div className={styles.imageError} data-bf-part="imageError" role="alert">{errorText}</div>
            ) : (
              <button
                aria-label={previewLabel}
                className={styles.imageButton}
                data-bf-part="imagePreview"
                onClick={onOpenPreview}
                type="button"
              >
                <img
                  alt={alt}
                  height={height}
                  onError={onImageError}
                  src={source}
                  width={width}
                />
              </button>
            )}
          </div>
        ) : undefined}
        header={(
          <AmbientToolCardHeader
            action={statusText}
            icon={<ToolCardStatusSlot status={status} toolIcon={<ImageIcon aria-hidden="true" />} />}
          />
        )}
        isExpanded={Boolean(source && isExpanded)}
        onClick={source && onToggle ? onToggle : undefined}
        status={status}
      />
      <Dialog
        open={Boolean(lightboxOpen && source && !imageFailed)}
        onOpenChange={(nextOpen) => {
          if (!nextOpen) onLightboxClose?.();
        }}
        size="lg"
      >
        <DialogHeader>
          <DialogHeading>
            <DialogTitle>{lightboxTitle ?? alt}</DialogTitle>
          </DialogHeading>
          <DialogClose />
        </DialogHeader>
        <DialogBody>
          <div className={styles.lightbox} data-bf-part="lightbox">
            <img alt={alt} src={source ?? ""} />
          </div>
        </DialogBody>
      </Dialog>
    </>
  );
}

export type TodoToolCardItemStatus = "cancelled" | "completed" | "in_progress" | "pending";

export interface TodoToolCardItem {
  content: ReactNode;
  key: string;
  status: TodoToolCardItemStatus;
}

export interface TodoToolCardProps extends AmbientCardProps {
  allCompleted?: boolean;
  compactCountLabel?: ReactNode;
  compactProgressLabel?: ReactNode;
  completedCount: number;
  items: readonly TodoToolCardItem[];
  loading?: boolean;
  mode?: "compact" | "standard";
  summary?: ReactNode;
  title: ReactNode;
  totalCount: number;
}

function TodoStatusIcon({ status }: { status: TodoToolCardItemStatus }) {
  if (status === "completed") return <CheckCircle2 aria-hidden="true" />;
  if (status === "cancelled") return <XCircle aria-hidden="true" />;
  if (status === "in_progress") return <ToolProcessingDots size={12} />;
  return <Circle aria-hidden="true" />;
}

export function TodoToolCard({
  allCompleted = false,
  compactCountLabel,
  compactProgressLabel,
  completedCount,
  isExpanded = false,
  items,
  loading = false,
  mode = "standard",
  onToggle,
  status,
  summary,
  title,
  totalCount,
  ...props
}: TodoToolCardProps) {
  if (mode === "compact") {
    return (
      <div
        {...props}
        className={styles.todoCompact}
        data-bf-state={[loading && "loading", allCompleted && "completed"].filter(Boolean).join(" ") || undefined}
        data-bf-tool-card="todo"
        data-bf-view="compact"
      >
        <span className={styles.todoCompactIcon}>
          {loading ? <ToolProcessingDots size={14} /> : <ListTodo aria-hidden="true" />}
        </span>
        {compactCountLabel && <span className={styles.todoCompactMeta}>{compactCountLabel}</span>}
        {compactProgressLabel && <span className={styles.todoCompactMeta}>{compactProgressLabel}</span>}
      </div>
    );
  }

  const hasItems = items.length > 0;
  const headerSummary = (
    <span className={styles.todoSummary} data-bf-part="summary">
      <span>{summary}</span>
      {hasItems && <span className={styles.todoStats}>({completedCount}/{totalCount})</span>}
    </span>
  );

  return (
    <AmbientToolCard
      {...props}
      data-bf-tool-card="todo"
      expandedContent={hasItems ? (
        <div className={styles.todoList} data-bf-part="todoList">
          {items.map((item) => (
            <div
              className={styles.todoItem}
              data-bf-part="todoItem"
              data-status={item.status}
              key={item.key}
            >
              <span className={styles.todoIcon}><TodoStatusIcon status={item.status} /></span>
              <span className={styles.todoContent}>{item.content}</span>
            </div>
          ))}
        </div>
      ) : undefined}
      header={(
        <AmbientToolCardHeader
          action={isExpanded ? undefined : title}
          content={headerSummary}
          icon={(
            <ToolCardStatusSlot
              defaultIcon={status === "error" || status === "cancelled" || allCompleted ? "status" : "tool"}
              status={allCompleted ? "completed" : status}
              toolIcon={<ListTodo aria-hidden="true" />}
            />
          )}
        />
      )}
      isExpanded={Boolean(isExpanded && hasItems)}
      onClick={hasItems && onToggle ? onToggle : undefined}
      status={status}
      toggleTestId="todo-tool-card-toggle"
    />
  );
}
