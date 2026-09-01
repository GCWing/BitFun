import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type HTMLAttributes,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { ArrowUpRight, ChevronDown } from "lucide-react";
import { classNames } from "../../internal/classNames";
import styles from "./FlowChatToolCard.module.css";

export type FlowChatToolStatus =
  | "pending"
  | "queued"
  | "waiting"
  | "preparing"
  | "streaming"
  | "receiving"
  | "running"
  | "completed"
  | "error"
  | "cancelled"
  | "rejected"
  | "analyzing"
  | "pending_confirmation"
  | "confirmed";

export type ToolCardHeaderAffordanceKind = "expand" | "open-panel-right";

const LOADING_STATUSES = new Set<FlowChatToolStatus>([
  "queued",
  "waiting",
  "preparing",
  "streaming",
  "receiving",
  "running",
  "analyzing",
]);

const TOOL_CARD_COLLAPSE_DURATION_MS = 300;

interface HeaderLayoutContextValue {
  affordanceKind: ToolCardHeaderAffordanceKind;
  attention: "ambient" | "prominent";
  expandable: boolean;
  isExpanded: boolean;
  onAffordanceClick?: (event: ReactMouseEvent<HTMLElement>) => void;
}

const HeaderLayoutContext = createContext<HeaderLayoutContextValue>({
  affordanceKind: "expand",
  attention: "ambient",
  expandable: false,
  isExpanded: false,
});

function getAppearanceState({
  isExpanded,
  isFailed,
  isLoading,
  requiresConfirmation,
}: {
  isExpanded: boolean;
  isFailed: boolean;
  isLoading: boolean;
  requiresConfirmation: boolean;
}): string | undefined {
  const states = [
    isExpanded && "expanded",
    isFailed && "failed",
    isLoading && "loading",
    requiresConfirmation && "confirmation",
  ].filter(Boolean);

  return states.length > 0 ? states.join(" ") : undefined;
}

function shouldIgnoreToggleClick(
  event: ReactMouseEvent<HTMLElement>,
  root: HTMLElement,
): boolean {
  if (event.defaultPrevented || event.button !== 0) {
    return true;
  }

  const target = event.target as { closest?: (selectors: string) => Element | null } | null;
  if (
    typeof target?.closest === "function" &&
    target.closest("button,a,input,textarea,select,[contenteditable='true'],[data-flow-card-ignore-toggle]")
  ) {
    return true;
  }

  const selection = root.ownerDocument.defaultView?.getSelection?.();
  if (!selection || selection.isCollapsed || !selection.toString().trim()) {
    return false;
  }

  const anchorInside = selection.anchorNode ? root.contains(selection.anchorNode) : false;
  const focusInside = selection.focusNode ? root.contains(selection.focusNode) : false;
  return anchorInside || focusInside;
}

interface CollapsibleRegionProps {
  children?: ReactNode;
  className?: string;
  disableAnimation?: boolean;
  isOpen: boolean;
  part: "error" | "expanded";
  status: FlowChatToolStatus;
}

type CollapsePhase = "closed" | "closing" | "open" | "opening";

function CollapsibleRegion({
  children,
  className,
  disableAnimation = false,
  isOpen,
  part,
  status,
}: CollapsibleRegionProps) {
  const hasContent = children !== undefined && children !== null && children !== false;
  const open = Boolean(isOpen && hasContent);
  const hasMountedRef = useRef(false);
  const [phase, setPhase] = useState<CollapsePhase>(() => (open ? "open" : "closed"));
  const [visuallyOpen, setVisuallyOpen] = useState(open);

  useEffect(() => {
    if (!hasMountedRef.current) {
      hasMountedRef.current = true;
      return;
    }

    const prefersReducedMotion =
      typeof window !== "undefined" &&
      (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false);
    const shouldAnimate = !disableAnimation && !prefersReducedMotion;

    if (!shouldAnimate) {
      setPhase(open ? "open" : "closed");
      setVisuallyOpen(open);
      return;
    }

    let frameId: number | undefined;
    let timeoutId: ReturnType<typeof setTimeout> | undefined;

    if (open) {
      setPhase((currentPhase) => currentPhase === "open" ? currentPhase : "opening");
      setVisuallyOpen(false);

      if (typeof window !== "undefined" && typeof window.requestAnimationFrame === "function") {
        frameId = window.requestAnimationFrame(() => setVisuallyOpen(true));
      } else {
        setVisuallyOpen(true);
      }

      timeoutId = setTimeout(() => setPhase("open"), TOOL_CARD_COLLAPSE_DURATION_MS);
    } else {
      setVisuallyOpen(false);
      setPhase((currentPhase) => currentPhase === "closed" ? currentPhase : "closing");
      timeoutId = setTimeout(() => setPhase("closed"), TOOL_CARD_COLLAPSE_DURATION_MS);
    }

    return () => {
      if (frameId !== undefined && typeof window !== "undefined") {
        window.cancelAnimationFrame(frameId);
      }
      if (timeoutId !== undefined) {
        clearTimeout(timeoutId);
      }
    };
  }, [disableAnimation, open]);

  if (!hasContent) {
    return null;
  }

  const shouldRender = open || phase !== "closed";

  return (
    <div
      aria-hidden={!open}
      className={styles.collapse}
      data-animate={disableAnimation ? "false" : "true"}
      data-bf-component="flow-chat-tool-card"
      data-bf-part={`${part}Collapse`}
      data-open={visuallyOpen ? "true" : "false"}
      data-phase={phase}
      style={{
        "--_tool-card-collapse-duration": `${TOOL_CARD_COLLAPSE_DURATION_MS}ms`,
      } as CSSProperties}
    >
      {shouldRender && (
        <div
          className={styles.collapseInner}
          data-bf-component="flow-chat-tool-card"
          data-bf-part="collapseInner"
        >
          <div
            className={classNames(
              part === "expanded" ? styles.expanded : styles.error,
              className,
            )}
            data-bf-component="flow-chat-tool-card"
            data-bf-part={part}
            data-bf-state={part === "error" ? "failed" : "expanded"}
            data-bf-status={status}
          >
            {children}
          </div>
        </div>
      )}
    </div>
  );
}

export interface ProminentToolCardProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "onClick"> {
  allowExpandedWhenFailed?: boolean;
  className?: string;
  disableExpandAnimation?: boolean;
  errorContent?: ReactNode;
  expandedContent?: ReactNode;
  header: ReactNode;
  headerAffordanceKind?: ToolCardHeaderAffordanceKind;
  headerExpandAffordance?: boolean;
  isExpanded?: boolean;
  isFailed?: boolean;
  onClick?: (event: ReactMouseEvent<HTMLElement>) => void;
  requiresConfirmation?: boolean;
  status: FlowChatToolStatus;
  toggleTestId?: string;
}

export function ProminentToolCard({
  allowExpandedWhenFailed = false,
  className,
  disableExpandAnimation = false,
  errorContent,
  expandedContent,
  header,
  headerAffordanceKind = "expand",
  headerExpandAffordance,
  isExpanded = false,
  isFailed = false,
  onClick,
  requiresConfirmation = false,
  status,
  toggleTestId,
  ...props
}: ProminentToolCardProps) {
  const failed = isFailed || status === "error";
  const expandable = headerExpandAffordance
    ?? Boolean(onClick && expandedContent && (!failed || allowExpandedWhenFailed));
  const loading = LOADING_STATUSES.has(status);
  const confirmation = requiresConfirmation && ![
    "completed",
    "confirmed",
    "cancelled",
    "rejected",
    "error",
  ].includes(status);
  const appearanceState = getAppearanceState({
    isExpanded,
    isFailed: failed,
    isLoading: loading,
    requiresConfirmation: confirmation,
  });

  const handleSurfaceClick = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (!onClick || shouldIgnoreToggleClick(event, event.currentTarget)) {
      return;
    }
    onClick(event);
  };

  return (
    <div
      {...props}
      className={classNames(
        styles.prominentRoot,
        className,
      )}
      data-bf-attention="prominent"
      data-bf-component="flow-chat-tool-card"
      data-bf-expandable={expandable ? "true" : "false"}
      data-bf-interactive={onClick ? "true" : "false"}
      data-bf-part="root"
      data-bf-state={appearanceState}
      data-bf-status={status}
    >
      <div
        className={classNames(
          styles.surface,
          styles.prominentSurface,
        )}
        data-bf-attention="prominent"
        data-bf-component="flow-chat-tool-card"
        data-bf-expandable={expandable ? "true" : "false"}
        data-bf-interactive={onClick ? "true" : "false"}
        data-bf-part="surface"
        data-bf-state={appearanceState}
        data-bf-status={status}
        data-testid={onClick ? toggleTestId : undefined}
        onClick={handleSurfaceClick}
      >
        <HeaderLayoutContext.Provider
          value={{
            affordanceKind: headerAffordanceKind,
            attention: "prominent",
            expandable,
            isExpanded,
            onAffordanceClick: onClick,
          }}
        >
          {header}
        </HeaderLayoutContext.Provider>
      </div>

      <CollapsibleRegion
        disableAnimation={disableExpandAnimation}
        isOpen={Boolean(isExpanded && expandedContent && (!failed || allowExpandedWhenFailed))}
        part="expanded"
        status={status}
      >
        {expandedContent}
      </CollapsibleRegion>

      <CollapsibleRegion
        isOpen={Boolean(failed && errorContent)}
        part="error"
        status={status}
      >
        {errorContent}
      </CollapsibleRegion>
    </div>
  );
}

export interface AmbientToolCardProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "onClick"> {
  className?: string;
  expandedContent?: ReactNode;
  header: ReactNode;
  isExpanded?: boolean;
  onClick?: (event: ReactMouseEvent<HTMLElement>) => void;
  status: FlowChatToolStatus;
  toggleTestId?: string;
}

export function AmbientToolCard({
  className,
  expandedContent,
  header,
  isExpanded = false,
  onClick,
  onKeyDown: onRootKeyDown,
  role,
  status,
  tabIndex,
  toggleTestId,
  ...props
}: AmbientToolCardProps) {
  const hasExpandedContent = Boolean(expandedContent);
  const collapseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [keepExpandedShell, setKeepExpandedShell] = useState(
    Boolean(isExpanded && hasExpandedContent),
  );

  useEffect(() => {
    if (collapseTimerRef.current !== null) {
      clearTimeout(collapseTimerRef.current);
      collapseTimerRef.current = null;
    }

    if (isExpanded && hasExpandedContent) {
      setKeepExpandedShell(true);
      return;
    }

    if (!hasExpandedContent) {
      setKeepExpandedShell(false);
      return;
    }

    if (keepExpandedShell) {
      collapseTimerRef.current = setTimeout(() => {
        collapseTimerRef.current = null;
        setKeepExpandedShell(false);
      }, TOOL_CARD_COLLAPSE_DURATION_MS);
    }
  }, [hasExpandedContent, isExpanded, keepExpandedShell]);

  useEffect(() => () => {
    if (collapseTimerRef.current !== null) {
      clearTimeout(collapseTimerRef.current);
    }
  }, []);

  const loading = LOADING_STATUSES.has(status);
  const expandedShell = keepExpandedShell || Boolean(isExpanded && hasExpandedContent);
  const interactive = Boolean(onClick);
  const directAction = interactive && !hasExpandedContent;
  const appearanceState = getAppearanceState({
    isExpanded,
    isFailed: status === "error",
    isLoading: loading,
    requiresConfirmation: false,
  });
  const expandable = interactive && hasExpandedContent;

  const handleSurfaceClick = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (!onClick || shouldIgnoreToggleClick(event, event.currentTarget)) {
      return;
    }
    onClick(event);
  };

  const handleDirectActionKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    onRootKeyDown?.(event);
    if (!directAction || event.defaultPrevented || (event.key !== "Enter" && event.key !== " ")) {
      return;
    }

    event.preventDefault();
    const surface = event.currentTarget.querySelector<HTMLElement>(
      '[data-bf-component="flow-chat-tool-card"][data-bf-part="surface"]',
    );
    surface?.click();
  };

  return (
    <div
      {...props}
      className={classNames(
        styles.ambientRoot,
        expandedShell && styles.ambientExpandedShell,
        className,
      )}
      data-bf-attention="ambient"
      data-bf-component="flow-chat-tool-card"
      data-bf-direct-action={directAction ? "true" : "false"}
      data-bf-expandable={expandable ? "true" : "false"}
      data-bf-interactive={interactive ? "true" : "false"}
      data-bf-part="root"
      data-bf-state={appearanceState}
      data-bf-status={status}
      data-bf-expanded-shell={expandedShell ? "true" : "false"}
      onKeyDown={directAction || onRootKeyDown ? handleDirectActionKeyDown : undefined}
      role={directAction ? "button" : role}
      tabIndex={directAction ? 0 : tabIndex}
    >
      <div
        className={classNames(
          styles.surface,
          styles.ambientSurface,
        )}
        data-bf-attention="ambient"
        data-bf-component="flow-chat-tool-card"
        data-bf-expandable={expandable ? "true" : "false"}
        data-bf-interactive={interactive ? "true" : "false"}
        data-bf-part="surface"
        data-bf-state={appearanceState}
        data-bf-status={status}
        data-testid={interactive ? toggleTestId : undefined}
        onClick={handleSurfaceClick}
      >
        <HeaderLayoutContext.Provider
          value={{
            affordanceKind: "expand",
            attention: "ambient",
            expandable,
            isExpanded,
            onAffordanceClick: onClick,
          }}
        >
          {header}
        </HeaderLayoutContext.Provider>
      </div>

      <CollapsibleRegion
        isOpen={Boolean(isExpanded && expandedContent)}
        part="expanded"
        status={status}
      >
        {expandedContent}
      </CollapsibleRegion>
    </div>
  );
}

export interface ToolCardIconSlotProps {
  affordanceKind?: ToolCardHeaderAffordanceKind;
  className?: string;
  expandable?: boolean;
  icon: ReactNode;
  isExpanded?: boolean;
  onAffordanceClick?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  showDivider?: boolean;
}

export function ToolCardIconSlot({
  affordanceKind,
  className,
  expandable,
  icon,
  isExpanded,
  onAffordanceClick,
  showDivider = false,
}: ToolCardIconSlotProps) {
  const layout = useContext(HeaderLayoutContext);
  const resolvedExpandable = expandable ?? layout.expandable;
  const resolvedKind = affordanceKind ?? layout.affordanceKind;
  const resolvedExpanded = isExpanded ?? layout.isExpanded;
  const handleAffordance = onAffordanceClick ?? layout.onAffordanceClick;
  const showInlineAffordance = layout.attention === "ambient" && resolvedExpandable;
  const isPanelAffordance = resolvedKind === "open-panel-right";

  return (
    <span
      className={classNames(
        styles.iconSlot,
        className,
      )}
      data-bf-affordance={resolvedKind}
      data-bf-component="flow-chat-tool-card"
      data-bf-expandable={showInlineAffordance ? "true" : "false"}
      data-bf-part="icon"
      data-divider={showDivider ? "true" : "false"}
    >
      <span
        className={styles.iconMarks}
        data-bf-component="flow-chat-tool-card"
        data-bf-part="iconMarks"
      >
        <span
          className={styles.mainIcon}
          data-bf-component="flow-chat-tool-card"
          data-bf-part="iconGraphic"
        >
          {icon}
        </span>
        {showInlineAffordance && (
          <span
            aria-hidden="true"
            className={styles.inlineAffordance}
            data-bf-affordance={resolvedKind}
            data-bf-component="flow-chat-tool-card"
            data-bf-part="iconAffordance"
            data-expanded={resolvedExpanded ? "true" : "false"}
          >
            {isPanelAffordance
              ? <ArrowUpRight aria-hidden="true" />
              : <ChevronDown aria-hidden="true" />}
          </span>
        )}
      </span>
      {showInlineAffordance && handleAffordance && (
        <button
          aria-expanded={isPanelAffordance ? undefined : resolvedExpanded}
          aria-label={isPanelAffordance ? "Open details" : resolvedExpanded ? "Collapse details" : "Expand details"}
          className={styles.iconAffordanceHit}
          data-bf-affordance={resolvedKind}
          data-bf-component="flow-chat-tool-card"
          data-bf-part="iconAffordanceButton"
          onClick={(event) => {
            event.stopPropagation();
            handleAffordance(event);
          }}
          type="button"
        />
      )}
    </span>
  );
}

export interface ToolCardStatusIconProps {
  className?: string;
  icon: ReactNode;
  withDivider?: boolean;
}

export function ToolCardStatusIcon({
  className,
  icon,
  withDivider = false,
}: ToolCardStatusIconProps) {
  return (
    <span
      className={classNames(styles.statusIcon, className)}
      data-bf-component="flow-chat-tool-card"
      data-bf-part="status"
      data-divider={withDivider ? "true" : "false"}
    >
      {icon}
    </span>
  );
}

export interface ToolCardHeaderActionsProps {
  children: ReactNode;
  className?: string;
}

export function ToolCardHeaderActions({ children, className }: ToolCardHeaderActionsProps) {
  return (
    <span
      className={classNames(styles.headerActions, className)}
      data-bf-component="flow-chat-tool-card"
      data-bf-part="actions"
      onClick={(event) => event.stopPropagation()}
    >
      {children}
    </span>
  );
}

export interface ToolCardChangeSummaryProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, "children"> {
  additions?: number | string;
  deletions?: number | string;
}

export function ToolCardChangeSummary({
  additions,
  className,
  deletions,
  ...props
}: ToolCardChangeSummaryProps) {
  const hasAdditions = additions !== undefined && additions !== null && additions !== "";
  const hasDeletions = deletions !== undefined && deletions !== null && deletions !== "";

  if (!hasAdditions && !hasDeletions) {
    return null;
  }

  return (
    <span
      {...props}
      className={classNames(styles.changeSummary, className)}
      data-bf-component="flow-chat-tool-card"
      data-bf-part="changeSummary"
    >
      {hasAdditions && (
        <span data-bf-change="added">+{additions}</span>
      )}
      {hasDeletions && (
        <span data-bf-change="removed">-{deletions}</span>
      )}
    </span>
  );
}

export interface ProminentToolCardHeaderProps {
  action?: ReactNode;
  actionDataAttributes?: Record<`data-${string}`, boolean | number | string | undefined>;
  actionTestId?: string;
  actions?: ReactNode;
  affordanceKind?: ToolCardHeaderAffordanceKind;
  content?: ReactNode;
  expandAffordance?: boolean;
  extra?: ReactNode;
  headerExpanded?: boolean;
  icon?: ReactNode;
  onAffordanceClick?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  statusIcon?: ReactNode;
  trailingActions?: ReactNode;
}

export function ProminentToolCardHeader({
  action,
  actionDataAttributes,
  actionTestId,
  actions,
  affordanceKind,
  content,
  expandAffordance,
  extra,
  headerExpanded,
  icon,
  onAffordanceClick,
  statusIcon,
  trailingActions,
}: ProminentToolCardHeaderProps) {
  const layout = useContext(HeaderLayoutContext);
  const expandable = expandAffordance ?? layout.expandable;
  const resolvedKind = affordanceKind ?? layout.affordanceKind;
  const expanded = headerExpanded ?? layout.isExpanded;
  const handleAffordance = onAffordanceClick ?? layout.onAffordanceClick;
  const affordanceAction = expandable ? handleAffordance : undefined;
  const hasActionRegion = Boolean(actions || affordanceAction || trailingActions);

  return (
    <div
      className={classNames(styles.header, styles.prominentHeader)}
      data-bf-affordance={resolvedKind}
      data-bf-component="flow-chat-tool-card"
      data-bf-expandable={expandable ? "true" : "false"}
      data-bf-part="header"
    >
      {icon !== undefined && icon !== null && icon !== false && icon !== "" && (
        <ToolCardIconSlot icon={icon} />
      )}
      {action !== undefined && action !== null && action !== false && action !== "" && (
        <span
          {...actionDataAttributes}
          className={styles.actionLabel}
          data-bf-component="flow-chat-tool-card"
          data-bf-part="action"
          data-testid={actionTestId}
        >
          {action}
        </span>
      )}
      {content !== undefined && content !== null && content !== false && (
        <span
          className={styles.content}
          data-bf-component="flow-chat-tool-card"
          data-bf-part="content"
        >
          {content}
        </span>
      )}
      {extra !== undefined && extra !== null && extra !== false && (
        <span
          className={styles.extra}
          data-bf-component="flow-chat-tool-card"
          data-bf-part="extra"
        >
          {extra}
        </span>
      )}
      {statusIcon !== undefined && statusIcon !== null && statusIcon !== false && (
        <ToolCardStatusIcon icon={statusIcon} withDivider={Boolean(extra)} />
      )}
      {hasActionRegion && (
        <span
          className={styles.actionRegion}
          data-bf-component="flow-chat-tool-card"
          data-bf-part="actionRegion"
        >
          {actions}
          {affordanceAction && (
            <button
              aria-expanded={resolvedKind === "expand" ? expanded : undefined}
              aria-label={
                resolvedKind === "open-panel-right"
                  ? "Open details"
                  : expanded
                    ? "Collapse details"
                    : "Expand details"
              }
              className={styles.affordanceButton}
              data-bf-affordance={resolvedKind}
              data-bf-component="flow-chat-tool-card"
              data-bf-part="affordanceButton"
              onClick={(event) => {
                event.stopPropagation();
                affordanceAction(event);
              }}
              type="button"
            >
              {resolvedKind === "open-panel-right"
                ? <ArrowUpRight aria-hidden="true" />
                : <ChevronDown aria-hidden="true" />}
            </button>
          )}
          {trailingActions !== undefined && trailingActions !== null && trailingActions !== false && (
            <span
              className={styles.trailingActions}
              data-bf-component="flow-chat-tool-card"
              data-bf-part="trailingActions"
              data-divider={affordanceAction ? "true" : "false"}
            >
              {trailingActions}
            </span>
          )}
        </span>
      )}
    </div>
  );
}

export interface AmbientToolCardHeaderProps {
  action?: ReactNode;
  affordanceKind?: ToolCardHeaderAffordanceKind;
  content?: ReactNode;
  expandable?: boolean;
  extra?: ReactNode;
  icon?: ReactNode;
  isExpanded?: boolean;
  onAffordanceClick?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  rightStatusIcon?: ReactNode;
  rightStatusIconWithDivider?: boolean;
  showDivider?: boolean;
}

export function AmbientToolCardHeader({
  action,
  affordanceKind = "expand",
  content,
  expandable,
  extra,
  icon,
  isExpanded,
  onAffordanceClick,
  rightStatusIcon,
  rightStatusIconWithDivider = false,
  showDivider = false,
}: AmbientToolCardHeaderProps) {
  const layout = useContext(HeaderLayoutContext);

  return (
    <>
      {icon !== undefined && icon !== null && icon !== false && icon !== "" && (
        <ToolCardIconSlot
          affordanceKind={affordanceKind}
          expandable={expandable ?? layout.expandable}
          icon={icon}
          isExpanded={isExpanded ?? layout.isExpanded}
          onAffordanceClick={onAffordanceClick}
          showDivider={showDivider}
        />
      )}
      {action !== undefined && action !== null && action !== false && action !== "" && (
        <span
          className={styles.ambientAction}
          data-bf-component="flow-chat-tool-card"
          data-bf-part="action"
        >
          {action}
        </span>
      )}
      {content !== undefined && content !== null && content !== false && (
        <span
          className={styles.ambientContent}
          data-bf-component="flow-chat-tool-card"
          data-bf-part="content"
        >
          {content}
        </span>
      )}
      {extra !== undefined && extra !== null && extra !== false && (
        <span
          className={styles.ambientExtra}
          data-bf-component="flow-chat-tool-card"
          data-bf-part="extra"
        >
          {extra}
        </span>
      )}
      {rightStatusIcon !== undefined && rightStatusIcon !== null && rightStatusIcon !== false && (
        <ToolCardStatusIcon
          icon={rightStatusIcon}
          withDivider={rightStatusIconWithDivider}
        />
      )}
    </>
  );
}
