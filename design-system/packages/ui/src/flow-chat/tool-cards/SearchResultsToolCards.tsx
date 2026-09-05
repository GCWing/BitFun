import type { HTMLAttributes, ReactNode } from "react";
import {
  File,
  Folder,
  FolderOpen,
  FolderSearch,
  Globe,
  Link,
  Search,
} from "lucide-react";
import {
  AmbientToolCard,
  AmbientToolCardHeader,
  type FlowChatToolStatus,
} from "./FlowChatToolCard";
import { ToolCardStatusSlot } from "./ToolCardStatusSlot";
import styles from "./SearchResultsToolCards.module.css";

export interface SearchToolCardDetail {
  label: ReactNode;
  value: ReactNode;
}

export interface SearchToolCardResult {
  description?: ReactNode;
  icon?: "directory" | "file" | "link";
  key: string;
  meta?: ReactNode;
  onOpen?: () => void;
  title: ReactNode;
  url?: string;
}

interface SearchResultsToolCardBaseProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "onClick" | "results"> {
  action?: ReactNode;
  details?: readonly SearchToolCardDetail[];
  icon: ReactNode;
  isExpanded?: boolean;
  moreResultsLabel?: ReactNode;
  onToggle?: () => void;
  resultText?: string;
  results?: readonly SearchToolCardResult[];
  status: FlowChatToolStatus;
  summary: ReactNode;
  toolCard: string;
}

function ResultIcon({ kind }: { kind?: SearchToolCardResult["icon"] }) {
  if (kind === "directory") return <Folder aria-hidden="true" />;
  if (kind === "link") return <Link aria-hidden="true" />;
  return <File aria-hidden="true" />;
}

function SearchResultsToolCardBase({
  action,
  details = [],
  icon,
  isExpanded = false,
  moreResultsLabel,
  onToggle,
  resultText,
  results = [],
  status,
  summary,
  toolCard,
  ...props
}: SearchResultsToolCardBaseProps) {
  const hasDetails = details.length > 0 || results.length > 0 || Boolean(resultText);
  const expandedContent = hasDetails ? (
    <div data-openbitfun-part="searchDetails">
      {details.length > 0 && (
        <div className={styles.details} data-openbitfun-part="details">
          {details.map((detail, index) => (
            <span
              className={styles.detail}
              data-openbitfun-part="detail"
              key={index}
            >
              <span className={styles.detailLabel}>{detail.label}</span>
              <span className={styles.detailValue}>{detail.value}</span>
            </span>
          ))}
        </div>
      )}

      {resultText && (
        <pre className={styles.resultText} data-openbitfun-part="resultText">{resultText}</pre>
      )}

      {results.length > 0 && (
        <div className={styles.results} data-openbitfun-part="results">
          {results.map((result) => (
            <div className={styles.result} data-openbitfun-part="result" key={result.key}>
              <span className={styles.resultIcon}><ResultIcon kind={result.icon} /></span>
              <span className={styles.resultBody}>
                {result.onOpen ? (
                  <button
                    className={styles.resultButton}
                    onClick={result.onOpen}
                    title={result.url}
                    type="button"
                  >
                    {result.title}
                  </button>
                ) : (
                  <span className={styles.resultTitle}>{result.title}</span>
                )}
                {result.description && (
                  <span className={styles.resultDescription}>{result.description}</span>
                )}
                {result.url && <span className={styles.resultUrl}>{result.url}</span>}
              </span>
              {result.meta && <span className={styles.resultMeta}>{result.meta}</span>}
            </div>
          ))}
          {moreResultsLabel && (
            <div className={styles.overflowLabel} data-openbitfun-part="overflowLabel">
              {moreResultsLabel}
            </div>
          )}
        </div>
      )}
    </div>
  ) : undefined;

  return (
    <AmbientToolCard
      {...props}
      data-openbitfun-tool-card={toolCard}
      expandedContent={expandedContent}
      header={(
        <AmbientToolCardHeader
          action={action}
          content={summary}
          icon={(
            <ToolCardStatusSlot
              status={status}
              toolIcon={icon}
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

export type SearchResultsToolCardProps = Omit<
  SearchResultsToolCardBaseProps,
  "icon" | "toolCard"
>;

export function GrepSearchToolCard(props: SearchResultsToolCardProps) {
  return <SearchResultsToolCardBase {...props} icon={<Search aria-hidden="true" />} toolCard="grep-search" />;
}

export function GlobSearchToolCard(props: SearchResultsToolCardProps) {
  return <SearchResultsToolCardBase {...props} icon={<FolderSearch aria-hidden="true" />} toolCard="glob-search" />;
}

export function DirectoryListToolCard(props: SearchResultsToolCardProps) {
  return <SearchResultsToolCardBase {...props} icon={<FolderOpen aria-hidden="true" />} toolCard="directory-list" />;
}

export function WebSearchToolCard(props: SearchResultsToolCardProps) {
  return <SearchResultsToolCardBase {...props} icon={<Globe aria-hidden="true" />} toolCard="web-search" />;
}
