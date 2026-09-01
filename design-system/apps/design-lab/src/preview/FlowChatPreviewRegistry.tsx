import {
  Fragment,
  useState,
  type ReactNode,
} from "react";
import {
  Archive,
  ArrowDownToLine,
  ArrowUp,
  Bot,
  CheckCircle2,
  Circle,
  Code2,
  Copy,
  FileEdit,
  FileText,
  FolderOpen,
  FolderSearch,
  GitBranch,
  GitCompare,
  Globe,
  Hourglass,
  Image as ImageIcon,
  Info,
  Layers,
  ListEnd,
  ListTodo,
  MessageSquare,
  Mic,
  Monitor,
  Plus,
  Pencil,
  Rocket,
  Search,
  SearchCheck,
  Shield,
  SquareTerminal,
  Terminal,
  Timer,
  Trash2,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { IconButton } from "@bitfun/ui";
import {
  AgentControlToolCard,
  AgentWaitToolCard,
  AmbientToolCard,
  AmbientToolCardHeader,
  AskUser,
  ChatComposer,
  ChatComposerActionButton,
  ChatComposerQueue,
  ChatComposerQueueAttachmentBadge,
  ChatComposerQueueHeader,
  ChatComposerQueueItem,
  ChatComposerQueueItemActions,
  ChatComposerQueueItemContent,
  ChatComposerQueueList,
  ChatComposerQueueTitle,
  CommandToolCard,
  ContextCompressionToolCard,
  DefaultToolCard,
  DirectoryListToolCard,
  FileDiffToolCard,
  FileOperationToolCard,
  GetToolSpecToolCard,
  GitToolCard,
  GlobSearchToolCard,
  GrepSearchToolCard,
  PageDeployToolCard,
  PagePublishToolCard,
  ProminentToolCard,
  ProminentToolCardHeader,
  ReadFileToolCard,
  ReviewSummaryToolCard,
  RunCodeToolCard,
  SessionControlToolCard,
  SessionMessageToolCard,
  SkillToolCard,
  TerminalControlToolCard,
  TodoToolCard,
  ToolCardChangeSummary,
  ToolCardHeaderActions,
  ViewImageToolCard,
  WebFetchToolCard,
  WebSearchToolCard,
  type FileOperationKind,
  type FlowChatToolStatus,
  type AskUserAnswers,
  type AskUserState,
} from "@bitfun/ui/flow-chat";
import { componentRegistry } from "@bitfun/ui/registry";
import { useI18n } from "../i18n";
import "./FlowChatPreviewRegistry.css";

type RegisteredComponent = (typeof componentRegistry)[number];

export type FlowChatComponentMeta = Extract<
  RegisteredComponent,
  { readonly category: "flow-chat" }
>;
export type FlowChatComponentName = FlowChatComponentMeta["name"];
export type FlowChatPreviewAttention = "adaptive" | "ambient" | "prominent";
export type FlowChatPreviewSection = "framework" | "tool-card";
export type FlowChatPreviewState = FlowChatComponentMeta["states"][number];

export interface FlowChatToolSpecimen {
  tool: string;
}

interface FlowChatPreviewRenderOptions {
  interactive: boolean;
  specimen?: FlowChatToolSpecimen;
  state: FlowChatPreviewState;
}

type Translate = ReturnType<typeof useI18n>["t"];

export interface FlowChatPreviewDefinition {
  attention: FlowChatPreviewAttention;
  codeSample: (t: Translate) => string;
  icon: LucideIcon;
  render: (options: FlowChatPreviewRenderOptions) => ReactNode;
  section: FlowChatPreviewSection;
  specimens: readonly FlowChatToolSpecimen[];
}

type FlowChatPreviewDefinitionMap = {
  readonly [Name in FlowChatComponentName]: FlowChatPreviewDefinition;
};

type PreviewProps = FlowChatPreviewRenderOptions;

function ChatComposerPreview({ interactive, state }: PreviewProps) {
  const [value, setValue] = useState("");
  const expanded = state === "expanded";
  const queued = state === "queued";

  return (
    <div className="flow-chat-composer-preview">
      <ChatComposer
        busy={state === "busy"}
        contextBar={(
          <div className="flow-chat-composer-preview__context">
            <span><Monitor aria-hidden="true" />This computer</span>
            <span>BitFun</span>
            <span><GitBranch aria-hidden="true" />1.0.0-explore</span>
            <span><Circle aria-hidden="true" />worktree</span>
            <span className="flow-chat-composer-preview__permission">
              <Shield aria-hidden="true" />Ask
            </span>
          </div>
        )}
        disabled={state === "disabled"}
        endActions={(
          <>
            <button
              className="flow-chat-composer-preview__model"
              disabled={state === "disabled"}
              type="button"
            >
              <span>deepseek-v4-pro</span>
              <small>high</small>
            </button>
            <ChatComposerActionButton
              aria-label="Voice input"
              disabled={state === "disabled"}
              icon={<Mic aria-hidden="true" />}
            />
            <ChatComposerActionButton
              aria-label="Send"
              disabled={state === "disabled"}
              icon={<ArrowUp aria-hidden="true" />}
              variant="primary"
            />
          </>
        )}
        layout={expanded ? "expanded" : "compact"}
        queue={queued ? (
          <ChatComposerQueue aria-label="Wait for sending">
            <ChatComposerQueueHeader>
              <ListEnd aria-hidden="true" />
              <ChatComposerQueueTitle count={13}>
                Wait for sending
              </ChatComposerQueueTitle>
            </ChatComposerQueueHeader>
            <ChatComposerQueueList>
              <ChatComposerQueueItem>
                <ChatComposerQueueItemContent>
                  Help me turn these two photos into Studio Ghibli style. Wait, maybe…
                </ChatComposerQueueItemContent>
                <ChatComposerQueueAttachmentBadge
                  count={3}
                  label="3 image attachments"
                />
                <ChatComposerQueueItemActions>
                  <IconButton
                    aria-label="Send now"
                    icon={<ArrowUp />}
                    size="xs"
                  />
                  <IconButton
                    aria-label="Delete"
                    icon={<Trash2 />}
                    size="xs"
                  />
                  <IconButton
                    aria-label="Edit"
                    icon={<Pencil />}
                    size="xs"
                  />
                </ChatComposerQueueItemActions>
              </ChatComposerQueueItem>
            </ChatComposerQueueList>
          </ChatComposerQueue>
        ) : undefined}
        startActions={(
          <ChatComposerActionButton
            aria-label="Add context"
            disabled={state === "disabled"}
            icon={<Plus aria-hidden="true" />}
            variant="fill"
          />
        )}
      >
        <textarea
          aria-label="Message"
          disabled={state === "disabled"}
          onChange={interactive ? (event) => setValue(event.target.value) : undefined}
          placeholder="How can I help you..."
          rows={expanded ? 3 : 1}
          value={value}
        />
      </ChatComposer>
    </div>
  );
}

function resolveStatus(state: FlowChatPreviewState): FlowChatToolStatus {
  if (state === "loading") return "running";
  if (state === "error") return "error";
  if (state === "confirmation") return "pending_confirmation";
  return "completed";
}

function FrameworkPreview({
  interactive,
  kind,
  state,
}: PreviewProps & { kind: "ambient" | "prominent" }) {
  const { t } = useI18n();
  const [isExpanded, setIsExpanded] = useState(state === "expanded");
  const status = resolveStatus(state);
  const previewState = state === "hover" ? "hover" : undefined;
  const toggleExpanded = interactive
    ? () => setIsExpanded((expanded) => !expanded)
    : undefined;
  const expandedContent = (
    <div className="flow-chat-tool-card-preview__details">
      <code>HTTP 200 · size=2,486 bytes</code>
      <span>{t("components.preview.flowChat.completed")}</span>
    </div>
  );

  if (kind === "ambient") {
    return (
      <div className="flow-chat-tool-card-preview" data-preview-kind="ambient">
        <AmbientToolCard
          data-bf-preview-state={previewState}
          expandedContent={expandedContent}
          header={(
            <AmbientToolCardHeader
              action={t("components.preview.flowChat.readFile")}
              content="src/flow_chat/tool-cards/index.ts"
              extra={state === "error"
                ? t("components.preview.flowChat.failed")
                : "128 lines"}
              icon={<FileText aria-hidden="true" />}
            />
          )}
          isExpanded={isExpanded}
          onClick={toggleExpanded}
          status={status}
        />
      </div>
    );
  }

  const actions = interactive ? (
    <ToolCardHeaderActions>
      <button
        aria-label={t("components.preview.flowChat.download")}
        className="flow-chat-tool-card-preview__action"
        type="button"
      >
        <ArrowDownToLine aria-hidden="true" />
      </button>
      <button
        aria-label={t("components.preview.flowChat.copy")}
        className="flow-chat-tool-card-preview__action"
        type="button"
      >
        <Copy aria-hidden="true" />
      </button>
    </ToolCardHeaderActions>
  ) : undefined;

  return (
    <div className="flow-chat-tool-card-preview" data-preview-kind="prominent">
      <ProminentToolCard
        data-bf-preview-state={previewState}
        errorContent={t("components.preview.flowChat.commandFailed")}
        expandedContent={expandedContent}
        header={(
          <ProminentToolCardHeader
            action={t("components.preview.flowChat.runCommand")}
            actions={actions}
            content={(
              <code className="flow-chat-tool-card-preview__command">
                {'curl -s -o /dev/null -w "HTTP %{http_code}" https://openbitfun.com'}
              </code>
            )}
            extra={(
              <ToolCardChangeSummary
                additions={6}
                aria-label={t("components.preview.flowChat.changeSummary")}
                deletions={0}
              />
            )}
            icon={<SquareTerminal aria-hidden="true" />}
          />
        )}
        isExpanded={isExpanded}
        onClick={toggleExpanded}
        requiresConfirmation={state === "confirmation"}
        status={status}
      />
    </div>
  );
}

function ReadFilePreview({ interactive, state }: PreviewProps) {
  const { t } = useI18n();
  const status = resolveStatus(state);
  const summary = state === "loading"
    ? t("components.preview.flowChat.readingFile")
    : state === "error"
      ? t("components.preview.flowChat.readFailed")
      : "src/flow_chat/tool-cards/index.ts · 128 lines";

  return (
    <div className="flow-chat-tool-card-preview">
      <ReadFileToolCard
        accessibleLabel={t("components.preview.flowChat.readFile")}
        data-bf-preview-state={state === "hover" ? "hover" : undefined}
        interactive={interactive}
        onOpen={interactive ? () => undefined : undefined}
        status={status}
        summary={summary}
      />
    </div>
  );
}

function ContextCompressionPreview({ state }: PreviewProps) {
  const { t } = useI18n();
  const status = resolveStatus(state);

  return (
    <div className="flow-chat-tool-card-preview">
      <ContextCompressionToolCard
        data-bf-preview-state={state === "hover" ? "hover" : undefined}
        error={state === "error"
          ? t("components.preview.flowChat.contextError")
          : undefined}
        processingText={status !== "error"
          ? t("components.preview.flowChat.contextProcessing")
          : undefined}
        status={status}
        summary={status === "completed"
          ? t("components.preview.flowChat.contextSummary")
          : undefined}
        title={t("components.preview.flowChat.contextCompression")}
      />
    </div>
  );
}

const COMMAND_SAMPLES = {
  Bash: {
    actionKey: "components.preview.flowChat.runCommand",
    command: "pnpm run design-system:check",
    output: "✓ packages built\n✓ public contracts verified",
  },
  ExecCommand: {
    actionKey: "components.preview.flowChat.runCommand",
    command: "pnpm --dir design-system --filter @bitfun/ui test",
    output: "57 tests passed",
  },
  ExecControl: {
    actionKey: "components.preview.flowChat.interruptProcess",
    command: "interrupt session 9182",
    output: "Process interrupted · exit 130",
  },
  WriteStdin: {
    actionKey: "components.preview.flowChat.pollProcess",
    command: "poll session 9182",
    output: "Process is still running",
  },
} as const;

function CommandPreview({ interactive, specimen, state }: PreviewProps) {
  const { t } = useI18n();
  const sample = COMMAND_SAMPLES[
    specimen?.tool && specimen.tool in COMMAND_SAMPLES
      ? specimen.tool as keyof typeof COMMAND_SAMPLES
      : "Bash"
  ];
  const [isExpanded, setIsExpanded] = useState(
    state === "expanded" || state === "loading",
  );
  const status = resolveStatus(state);
  const loading = state === "loading";
  const completed = status === "completed";
  const footerItems = completed ? [
    {
      label: t("components.preview.flowChat.exitCode"),
      monospace: true,
      tone: "success" as const,
      value: "0",
    },
    {
      label: t("components.preview.flowChat.duration"),
      value: "1.24s",
    },
  ] : [];
  const statusSummary = completed || loading ? (
    <span
      className="flow-chat-tool-card-preview__duration"
      data-status={completed ? "completed" : "running"}
    >
      {completed
        ? <CheckCircle2 aria-hidden="true" />
        : <Timer aria-hidden="true" />}
      <span>{completed ? "1.24s" : "1.2s"}</span>
    </span>
  ) : undefined;

  return (
    <div className="flow-chat-tool-card-preview">
      <CommandToolCard
        action={t(sample.actionKey)}
        command={sample.command}
        copyAction={interactive ? {
          label: t("components.preview.flowChat.copy"),
          onPress: () => undefined,
        } : undefined}
        data-bf-preview-state={state === "hover" ? "hover" : undefined}
        emptyCommand={t("components.preview.flowChat.emptyCommand")}
        error={state === "error"
          ? t("components.preview.flowChat.commandFailed")
          : undefined}
        footerItems={footerItems}
        isExpanded={isExpanded}
        onToggle={interactive ? () => setIsExpanded((expanded) => !expanded) : undefined}
        output={completed ? (
          <pre className="flow-chat-tool-card-preview__output">{sample.output}</pre>
        ) : undefined}
        outputDensity={loading ? "compact" : "expanded"}
        requiresConfirmation={state === "confirmation"}
        status={status}
        statusLabel={state === "error"
          ? t("components.preview.flowChat.failed")
          : undefined}
        statusSummary={statusSummary}
        statusTone={state === "error" ? "danger" : undefined}
        waitingContent={loading
          ? t("components.preview.flowChat.commandWaiting")
          : undefined}
      />
    </div>
  );
}

function resolveFileOperation(tool?: string): FileOperationKind {
  if (tool === "Delete") return "delete";
  if (tool === "Write") return "write";
  return "edit";
}

function FileOperationPreview({ interactive, specimen, state }: PreviewProps) {
  const { t } = useI18n();
  const operation = resolveFileOperation(specimen?.tool);
  const [isExpanded, setIsExpanded] = useState(state === "expanded");
  const status = resolveStatus(state);
  const path = operation === "write"
    ? "src/flow_chat/tool-cards/NewCard.tsx"
    : operation === "delete"
      ? "dist/stale-preview.js"
      : "src/flow_chat/components/FlowToolCard.tsx";
  const actionLabel = operation === "write"
    ? t("components.preview.flowChat.writeFile")
    : operation === "delete"
      ? t("components.preview.flowChat.deleteFile")
      : t("components.preview.flowChat.editFile");
  const preview = operation === "delete" ? undefined : (
    <pre className="flow-chat-tool-card-preview__diff">
      <span data-change="removed">- legacyToolCard</span>
      <span data-change="added">+ {operation === "write" ? "NewCard" : "FlowChatComponentPreview"}</span>
    </pre>
  );

  return (
    <div className="flow-chat-tool-card-preview">
      <FileOperationToolCard
        actionLabel={actionLabel}
        changeSummary={operation === "delete" ? undefined : {
          additions: operation === "write" ? 86 : 12,
          deletions: operation === "write" ? 0 : 4,
          label: t("components.preview.flowChat.fileChangeSummary"),
        }}
        data-bf-preview-state={state === "hover" ? "hover" : undefined}
        error={state === "error" ? {
          message: t("components.preview.flowChat.fileErrorMessage"),
          title: t("components.preview.flowChat.fileErrorTitle"),
        } : undefined}
        isExpanded={isExpanded}
        onOpenFile={interactive && operation !== "delete" ? {
          label: t("components.preview.flowChat.openFile"),
          onPress: () => undefined,
        } : undefined}
        onToggle={interactive ? () => setIsExpanded((expanded) => !expanded) : undefined}
        operation={operation}
        path={path}
        pathLabel={path}
        preview={preview}
        requiresConfirmation={state === "confirmation"}
        status={status}
        statusDetail={state === "loading"
          ? t("components.preview.flowChat.fileLoading")
          : undefined}
      />
    </div>
  );
}

type ActivityPreviewKind = "agent-wait" | "get-tool-spec" | "skill" | "terminal-control";

function ActivityPreview({
  kind,
  state,
}: PreviewProps & { kind: ActivityPreviewKind }) {
  const { t } = useI18n();
  const status = resolveStatus(state);
  const shared = {
    "data-bf-preview-state": state === "hover" ? "hover" : undefined,
    status,
    summary: state === "error"
      ? t("components.preview.flowChat.failed")
      : state === "loading"
        ? t("components.preview.flowChat.running")
        : t("components.preview.flowChat.completed"),
  };

  const card = kind === "agent-wait"
    ? <AgentWaitToolCard {...shared} action="AgentWait" />
    : kind === "get-tool-spec"
      ? <GetToolSpecToolCard {...shared} action="GetToolSpec" />
      : kind === "skill"
        ? <SkillToolCard {...shared} action="Skill" />
        : <TerminalControlToolCard {...shared} action="TerminalControl" />;

  return <div className="flow-chat-tool-card-preview">{card}</div>;
}

type SearchPreviewKind = "directory" | "glob" | "grep" | "web";

function SearchPreview({
  interactive,
  kind,
  state,
}: PreviewProps & { kind: SearchPreviewKind }) {
  const { t } = useI18n();
  const [isExpanded, setIsExpanded] = useState(state === "expanded");
  const status = resolveStatus(state);
  const Component = kind === "directory"
    ? DirectoryListToolCard
    : kind === "glob"
      ? GlobSearchToolCard
      : kind === "grep"
        ? GrepSearchToolCard
        : WebSearchToolCard;
  const summary = state === "loading"
    ? t("components.preview.flowChat.running")
    : state === "error"
      ? t("components.preview.flowChat.failed")
      : kind === "directory"
        ? "List src/ · 18 entries"
        : kind === "glob"
          ? "Find **/*.tsx · 42 files"
          : kind === "grep"
            ? "Search ToolCard · 27 matches"
            : "Search FlowChat component architecture · 3 results";

  return (
    <div className="flow-chat-tool-card-preview">
      <Component
        data-bf-preview-state={state === "hover" ? "hover" : undefined}
        details={[
          { label: "Scope", monospace: true, value: "src/flow_chat" },
          { label: "Results", value: "3" },
        ]}
        isExpanded={isExpanded}
        onToggle={interactive ? () => setIsExpanded((value) => !value) : undefined}
        resultText={kind === "grep" ? "FlowToolCard.tsx:42\nindex.ts:88\nREADME.md:17" : undefined}
        results={kind === "grep" ? undefined : [
          {
            description: kind === "web" ? "Reusable FlowChat card anatomy and migration boundary." : undefined,
            icon: kind === "web" ? "link" : "file",
            key: "one",
            onOpen: interactive && kind === "web" ? () => undefined : undefined,
            title: kind === "web" ? "FlowChat tool cards" : "src/flow_chat/tool-cards/index.ts",
            url: kind === "web" ? "https://openbitfun.com/docs/flow-chat" : undefined,
          },
          {
            icon: kind === "directory" ? "directory" : "file",
            key: "two",
            title: kind === "directory" ? "components/" : "design-system/packages/ui/src/flow-chat.ts",
          },
        ]}
        status={status}
        summary={summary}
      />
    </div>
  );
}

function SessionPreview({
  interactive,
  kind,
  state,
}: PreviewProps & { kind: "control" | "message" }) {
  const { t } = useI18n();
  const [isExpanded, setIsExpanded] = useState(state === "expanded");
  const status = resolveStatus(state);
  const common = {
    "data-bf-preview-state": state === "hover" ? "hover" : undefined,
    action: kind === "control" ? "SessionControl" : "SessionMessage",
    error: state === "error" ? t("components.preview.flowChat.failed") : undefined,
    fields: [
      { label: "Session", monospace: true, value: "review-42" },
      { label: "Workspace", monospace: true, value: "BitFun" },
    ],
    isExpanded,
    onToggle: interactive ? () => setIsExpanded((value) => !value) : undefined,
    status,
    summary: state === "loading"
      ? t("components.preview.flowChat.running")
      : t("components.preview.flowChat.completed"),
  };

  return (
    <div className="flow-chat-tool-card-preview">
      {kind === "control" ? (
        <SessionControlToolCard
          {...common}
          sessions={[{ agentType: "review", id: "review-42", key: "review-42", name: "UI audit" }]}
        />
      ) : (
        <SessionMessageToolCard
          {...common}
          message="Please verify the public FlowChat component contract."
          messageLabel="Message"
        />
      )}
    </div>
  );
}

const PREVIEW_IMAGE = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='320' height='320' viewBox='0 0 320 320'%3E%3Crect width='320' height='320' rx='32' fill='%23232a35'/%3E%3Cpath d='M60 235l64-76 42 45 31-36 63 67z' fill='%237e8ca3'/%3E%3Ccircle cx='224' cy='91' r='25' fill='%23c6cfdb'/%3E%3C/svg%3E";

type StandardAmbientPreviewKind = "default" | "run-code" | "todo" | "view-image" | "web-fetch";

function StandardAmbientPreview({
  interactive,
  kind,
  specimen,
  state,
}: PreviewProps & { kind: StandardAmbientPreviewKind }) {
  const { t } = useI18n();
  const [isExpanded, setIsExpanded] = useState(state === "expanded");
  const status = resolveStatus(state);
  const common = {
    "data-bf-preview-state": state === "hover" ? "hover" : undefined,
    isExpanded,
    onToggle: interactive ? () => setIsExpanded((value) => !value) : undefined,
    status,
  };

  let card: ReactNode;
  if (kind === "run-code") {
    card = (
      <RunCodeToolCard
        {...common}
        action="RunCode"
        error={state === "error" ? t("components.preview.flowChat.commandFailed") : undefined}
        output="packages built\ncontracts verified"
        outputLabel="Output"
        program={<pre className="flow-chat-tool-card-preview__output">await verifyDesignSystem();</pre>}
        programLabel="Program"
        summary="Verify the independent FlowChat package"
      />
    );
  } else if (kind === "web-fetch") {
    card = (
      <WebFetchToolCard
        {...common}
        action="WebFetch"
        content="FlowChat components expose stable semantic props and public data contracts."
        details={["markdown", "96 chars"]}
        error={state === "error" ? t("components.preview.flowChat.failed") : undefined}
        onOpenUrl={interactive ? () => undefined : undefined}
        openUrlLabel="Open source"
        title="FlowChat architecture"
        url="https://openbitfun.com/docs/flow-chat"
      />
    );
  } else if (kind === "default") {
    const toolName = specimen?.tool ?? "UnregisteredTool";
    card = (
      <DefaultToolCard
        {...common}
        description="Standard fallback presentation"
        displayName={toolName}
        error={state === "error" ? t("components.preview.flowChat.failed") : undefined}
        inputLabel="Input"
        inputPreview={'{\n  "scope": "flow-chat"\n}'}
        requiresConfirmation={state === "confirmation"}
        resultLabel="Result"
        resultPreview="Contract verified"
        summary={state === "confirmation" ? "Waiting for confirmation" : "Contract verified"}
        toolName={toolName}
      />
    );
  } else if (kind === "view-image") {
    card = (
      <ViewImageToolCard
        {...common}
        alt="FlowChat preview sample"
        errorText={t("components.preview.flowChat.failed")}
        imageFailed={state === "error"}
        previewLabel="Open image preview"
        source={state === "loading" ? undefined : PREVIEW_IMAGE}
        statusText={state === "loading" ? t("components.preview.flowChat.running") : "Viewed image"}
      />
    );
  } else {
    card = (
      <TodoToolCard
        {...common}
        allCompleted={false}
        completedCount={1}
        items={[
          { content: "Define public component", key: "one", status: "completed" },
          { content: "Migrate FlowChat adapter", key: "two", status: "in_progress" },
          { content: "Remove legacy CSS", key: "three", status: "pending" },
        ]}
        loading={state === "loading"}
        mode="standard"
        summary="Migrate FlowChat adapter"
        title="Tasks"
        totalCount={3}
      />
    );
  }

  return <div className="flow-chat-tool-card-preview">{card}</div>;
}

type ProminentPreviewKind = "agent" | "diff" | "git" | "page-deploy" | "page-publish" | "review";

function ConcreteProminentPreview({
  interactive,
  kind,
  state,
}: PreviewProps & { kind: ProminentPreviewKind }) {
  const { t } = useI18n();
  const [isExpanded, setIsExpanded] = useState(state === "expanded");
  const status = resolveStatus(state);
  const common = {
    "data-bf-preview-state": state === "hover" ? "hover" : undefined,
    isExpanded,
    onToggle: interactive ? () => setIsExpanded((value) => !value) : undefined,
    status,
  };

  let card: ReactNode;
  if (kind === "agent") {
    card = (
      <AgentControlToolCard
        {...common}
        agentName="reviewer"
        openAgentLabel="Open agent"
        prompt={<p>Review the FlowChat migration boundary and public contracts.</p>}
        statusLabel={state === "loading" ? t("components.preview.flowChat.running") : t("components.preview.flowChat.completed")}
        statusTone={state === "error" ? "danger" : state === "loading" ? "success" : "neutral"}
      />
    );
  } else if (kind === "diff") {
    card = (
      <FileDiffToolCard
        {...common}
        action="GetFileDiff:"
        changeSummary={{
          additions: 12,
          deletions: 4,
          label: t("components.preview.flowChat.fileChangeSummary"),
        }}
        error={state === "error" ? t("components.preview.flowChat.failed") : undefined}
        loading={state === "loading"}
        message="Public component migration"
        path="src/flow_chat/components/FlowToolCard.tsx"
        pathLabel="FlowToolCard.tsx"
        textPreview="- legacy shell\n+ public component"
      />
    );
  } else if (kind === "git") {
    card = (
      <GitToolCard
        {...common}
        action="Git:"
        command="git diff --stat"
        error={state === "error" ? t("components.preview.flowChat.commandFailed") : undefined}
        footerItems={[{ label: "Exit", monospace: true, tone: "success", value: "0" }]}
        loading={state === "loading"}
        statusSummary="21 files changed"
        stdout="21 files changed, 842 insertions(+), 517 deletions(-)"
      />
    );
  } else if (kind === "review") {
    card = (
      <ReviewSummaryToolCard
        {...common}
        changedFiles={["FlowToolCard.tsx", "registry.ts"]}
        fileCountLabel="2 files"
        filesLabel="Changed files"
        kind="review"
        loading={state === "loading"}
        summary="No blocking issues found in the component boundary."
        title="Review: 0 issues"
      />
    );
  } else {
    const Component = kind === "page-deploy" ? PageDeployToolCard : PagePublishToolCard;
    card = (
      <Component
        {...common}
        action={kind === "page-deploy" ? "PageDeploy:" : "PagePublish:"}
        error={state === "error" ? t("components.preview.flowChat.failed") : undefined}
        fields={[
          { label: "Slug", monospace: true, value: "flow-chat-cards" },
          { label: "Version", monospace: true, value: "v42" },
        ]}
        loading={state === "loading"}
        subject="flow-chat-cards"
        version="v42"
      />
    );
  }

  return <div className="flow-chat-tool-card-preview">{card}</div>;
}

function concreteCodeSample(name: string) {
  return () => `import { ${name} } from "@bitfun/ui/flow-chat";\n\n<${name} status="completed" />`;
}

function AskUserPreview({ interactive, state }: PreviewProps) {
  const { t } = useI18n();
  const completed = state === "completed" || state === "expanded";
  const startsSelected = completed
    || state === "selected"
    || state === "submitting"
    || state === "disabled";
  const [answers, setAnswers] = useState<AskUserAnswers>(
    startsSelected ? { version: ["beta"] } : {},
  );
  const [customAnswers, setCustomAnswers] = useState<Record<string, string>>({});
  const [isExpanded, setIsExpanded] = useState(state === "expanded");
  const componentState: AskUserState = state === "loading"
    ? "loading"
    : state === "error"
      ? "error"
      : state === "submitting"
        ? "submitting"
        : completed
          ? "completed"
          : "asking";
  const questions = [{
    customOption: {
      description: t("components.preview.flowChat.askUserOtherDescription"),
      inputLabel: t("components.preview.flowChat.askUserOtherInputLabel"),
      label: t("components.preview.flowChat.askUserOther"),
      placeholder: t("components.preview.flowChat.askUserOtherPlaceholder"),
      value: "other",
    },
    id: "version",
    options: [
      {
        description: t("components.preview.flowChat.askUserBetaDescription"),
        label: "v0.2.19-beta.1 (Recommended)",
        value: "beta",
      },
      {
        description: t("components.preview.flowChat.askUserStableDescription"),
        label: "v0.2.18",
        value: "stable",
      },
      {
        description: t("components.preview.flowChat.askUserNightlyDescription"),
        label: t("components.preview.flowChat.askUserNightly"),
        value: "nightly",
      },
    ],
    prompt: t("components.preview.flowChat.askUserQuestion"),
    selectionMode: "single" as const,
  }];
  const selectedValue = answers.version?.[0];
  const selectedLabel = selectedValue === "stable"
    ? "v0.2.18"
    : selectedValue === "nightly"
      ? t("components.preview.flowChat.askUserNightly")
      : selectedValue === "other"
        ? customAnswers.version || t("components.preview.flowChat.askUserOther")
        : "v0.2.19-beta.1 (Recommended)";
  const statusLabel = componentState === "loading"
    ? t("components.preview.flowChat.askUserLoading")
    : componentState === "error"
      ? t("components.preview.flowChat.askUserError")
      : componentState === "submitting"
        ? t("components.preview.flowChat.askUserSubmitting")
        : t("components.preview.flowChat.askUserWaiting");

  return (
    <div className="flow-chat-tool-card-preview">
      <AskUser
        answers={answers}
        customAnswers={customAnswers}
        disabled={state === "disabled"}
        expanded={completed ? isExpanded : undefined}
        header={completed ? undefined : t("components.preview.flowChat.askUserHeader")}
        onAnswersChange={interactive ? (questionId, values) => {
          setAnswers((current) => ({ ...current, [questionId]: values }));
        } : undefined}
        onCustomAnswerChange={interactive ? (questionId, value) => {
          setCustomAnswers((current) => ({ ...current, [questionId]: value }));
        } : undefined}
        onExpandedChange={interactive ? setIsExpanded : undefined}
        onSubmit={interactive ? () => undefined : undefined}
        questions={componentState === "error" ? [] : questions}
        state={componentState}
        statusLabel={completed ? undefined : statusLabel}
        submitDisabled={!answers.version?.length}
        submitLabel={completed || componentState === "loading" || componentState === "error"
          ? undefined
          : t("components.preview.flowChat.askUserSubmit")}
        submittingLabel={t("components.preview.flowChat.askUserSubmitting")}
        summaryDetail={completed
          ? `${t("components.preview.flowChat.askUserSummaryPrefix")}: ${selectedLabel}`
          : undefined}
        summaryLabel={completed
          ? t("components.preview.flowChat.askUserAnswered")
          : undefined}
      />
    </div>
  );
}

export const flowChatPreviewDefinitions = {
  AgentControlToolCard: {
    attention: "prominent",
    codeSample: concreteCodeSample("AgentControlToolCard"),
    icon: Bot,
    render: (options) => <ConcreteProminentPreview {...options} kind="agent" />,
    section: "tool-card",
    specimens: [
      { tool: "AgentSpawn" },
      { tool: "AgentSendInput" },
    ],
  },
  AgentWaitToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("AgentWaitToolCard"),
    icon: Hourglass,
    render: (options) => <ActivityPreview {...options} kind="agent-wait" />,
    section: "tool-card",
    specimens: [{ tool: "AgentWait" }],
  },
  AmbientToolCard: {
    attention: "ambient",
    codeSample: (t) => `import { AmbientToolCard, AmbientToolCardHeader } from "@bitfun/ui/flow-chat";\nimport { FileText } from "lucide-react";\n\n<AmbientToolCard\n  status="completed"\n  header={(\n    <AmbientToolCardHeader\n      icon={<FileText />}\n      action="${t("components.preview.flowChat.readFile")}"\n      content="src/flow_chat/tool-cards/index.ts"\n    />\n  )}\n/>`,
    icon: FileText,
    render: (options) => <FrameworkPreview {...options} kind="ambient" />,
    section: "framework",
    specimens: [],
  },
  AskUser: {
    attention: "prominent",
    codeSample: (t) => `import { AskUser } from "@bitfun/ui/flow-chat";\n\n<AskUser\n  answers={answers}\n  expanded\n  onAnswersChange={setAnswer}\n  questions={questions}\n  state="completed"\n  summaryLabel="${t("components.preview.flowChat.askUserAnswered")}"\n  summaryDetail="${t("components.preview.flowChat.askUserSummaryPrefix")}: v0.2.19-beta.1 (Recommended)"\n/>`,
    icon: MessageSquare,
    render: (options) => <AskUserPreview {...options} />,
    section: "tool-card",
    specimens: [{ tool: "AskUserQuestion" }],
  },
  ChatComposer: {
    attention: "adaptive",
    codeSample: () => `import { ChatComposer } from "@bitfun/ui/flow-chat";\n\n<ChatComposer\n  contextBar={<WorkspaceContext />}\n  queue={pendingMessages.length ? <PendingMessageQueue items={pendingMessages} /> : undefined}\n  layout={multiline ? "expanded" : "compact"}\n  startActions={<AddMenu />}\n  endActions={<ComposerActions />}\n>\n  <RichTextEditor />\n</ChatComposer>`,
    icon: MessageSquare,
    render: (options) => <ChatComposerPreview {...options} />,
    section: "framework",
    specimens: [],
  },
  CommandToolCard: {
    attention: "prominent",
    codeSample: (t) => `import { CommandToolCard } from "@bitfun/ui/flow-chat";\n\n<CommandToolCard\n  action="${t("components.preview.flowChat.runCommand")}"\n  command="pnpm run design-system:check"\n  emptyCommand="${t("components.preview.flowChat.emptyCommand")}"\n  isExpanded={isExpanded}\n  onToggle={() => setIsExpanded((value) => !value)}\n  output={<TerminalOutput />}\n  status="completed"\n/>`,
    icon: Terminal,
    render: (options) => <CommandPreview {...options} />,
    section: "tool-card",
    specimens: [
      { tool: "Bash" },
      { tool: "ExecCommand" },
      { tool: "WriteStdin" },
      { tool: "ExecControl" },
    ],
  },
  ContextCompressionToolCard: {
    attention: "prominent",
    codeSample: (t) => `import { ContextCompressionToolCard } from "@bitfun/ui/flow-chat";\n\n<ContextCompressionToolCard\n  status="completed"\n  summary="${t("components.preview.flowChat.contextSummary")}"\n  title="${t("components.preview.flowChat.contextCompression")}"\n/>`,
    icon: Archive,
    render: (options) => <ContextCompressionPreview {...options} />,
    section: "tool-card",
    specimens: [{ tool: "ContextCompression" }],
  },
  DefaultToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("DefaultToolCard"),
    icon: Info,
    render: (options) => <StandardAmbientPreview {...options} kind="default" />,
    section: "tool-card",
    specimens: [
      { tool: "ControlHub" },
      { tool: "FinalizeMiniApp" },
      { tool: "PublishMiniApp" },
      { tool: "PublishAppearance" },
      { tool: "UnregisteredTool" },
    ],
  },
  DirectoryListToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("DirectoryListToolCard"),
    icon: FolderOpen,
    render: (options) => <SearchPreview {...options} kind="directory" />,
    section: "tool-card",
    specimens: [{ tool: "LS" }],
  },
  FileDiffToolCard: {
    attention: "prominent",
    codeSample: (t) => `import { FileDiffToolCard } from "@bitfun/ui/flow-chat";\n\n<FileDiffToolCard\n  action="GetFileDiff:"\n  changeSummary={{\n    additions: 12,\n    deletions: 4,\n    label: "${t("components.preview.flowChat.fileChangeSummary")}",\n  }}\n  path="src/flow_chat/components/FlowToolCard.tsx"\n  pathLabel="FlowToolCard.tsx"\n  status="completed"\n/>`,
    icon: GitCompare,
    render: (options) => <ConcreteProminentPreview {...options} kind="diff" />,
    section: "tool-card",
    specimens: [{ tool: "GetFileDiff" }],
  },
  FileOperationToolCard: {
    attention: "adaptive",
    codeSample: (t) => `import { FileOperationToolCard } from "@bitfun/ui/flow-chat";\n\n<FileOperationToolCard\n  actionLabel="${t("components.preview.flowChat.editFile")}"\n  isExpanded={isExpanded}\n  onToggle={() => setIsExpanded((value) => !value)}\n  operation="edit"\n  path="src/flow_chat/components/FlowToolCard.tsx"\n  pathLabel="FlowToolCard.tsx"\n  preview={<DiffPreview />}\n  status="completed"\n/>`,
    icon: FileEdit,
    render: (options) => <FileOperationPreview {...options} />,
    section: "tool-card",
    specimens: [
      { tool: "Write" },
      { tool: "Edit" },
      { tool: "Delete" },
    ],
  },
  GetToolSpecToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("GetToolSpecToolCard"),
    icon: SearchCheck,
    render: (options) => <ActivityPreview {...options} kind="get-tool-spec" />,
    section: "tool-card",
    specimens: [{ tool: "GetToolSpec" }],
  },
  GitToolCard: {
    attention: "prominent",
    codeSample: concreteCodeSample("GitToolCard"),
    icon: GitBranch,
    render: (options) => <ConcreteProminentPreview {...options} kind="git" />,
    section: "tool-card",
    specimens: [{ tool: "Git" }],
  },
  GlobSearchToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("GlobSearchToolCard"),
    icon: FolderSearch,
    render: (options) => <SearchPreview {...options} kind="glob" />,
    section: "tool-card",
    specimens: [{ tool: "Glob" }],
  },
  GrepSearchToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("GrepSearchToolCard"),
    icon: Search,
    render: (options) => <SearchPreview {...options} kind="grep" />,
    section: "tool-card",
    specimens: [{ tool: "Grep" }],
  },
  PageDeployToolCard: {
    attention: "prominent",
    codeSample: concreteCodeSample("PageDeployToolCard"),
    icon: ArrowDownToLine,
    render: (options) => <ConcreteProminentPreview {...options} kind="page-deploy" />,
    section: "tool-card",
    specimens: [{ tool: "PageDeploy" }],
  },
  PagePublishToolCard: {
    attention: "prominent",
    codeSample: concreteCodeSample("PagePublishToolCard"),
    icon: Rocket,
    render: (options) => <ConcreteProminentPreview {...options} kind="page-publish" />,
    section: "tool-card",
    specimens: [{ tool: "PagePublish" }],
  },
  ProminentToolCard: {
    attention: "prominent",
    codeSample: (t) => `import { ProminentToolCard, ProminentToolCardHeader } from "@bitfun/ui/flow-chat";\nimport { SquareTerminal } from "lucide-react";\n\n<ProminentToolCard\n  status="completed"\n  isExpanded={isExpanded}\n  onClick={() => setIsExpanded((value) => !value)}\n  header={(\n    <ProminentToolCardHeader\n      icon={<SquareTerminal />}\n      action="${t("components.preview.flowChat.runCommand")}"\n      content={<code>curl https://openbitfun.com</code>}\n    />\n  )}\n  expandedContent={<CommandOutput />}\n/>`,
    icon: SquareTerminal,
    render: (options) => <FrameworkPreview {...options} kind="prominent" />,
    section: "framework",
    specimens: [],
  },
  ReadFileToolCard: {
    attention: "ambient",
    codeSample: (t) => `import { ReadFileToolCard } from "@bitfun/ui/flow-chat";\n\n<ReadFileToolCard\n  accessibleLabel="${t("components.preview.flowChat.readFile")}"\n  interactive\n  onOpen={openFile}\n  status="completed"\n  summary="src/flow_chat/tool-cards/index.ts · 128 lines"\n/>`,
    icon: FileText,
    render: (options) => <ReadFilePreview {...options} />,
    section: "tool-card",
    specimens: [{ tool: "Read" }],
  },
  ReviewSummaryToolCard: {
    attention: "prominent",
    codeSample: concreteCodeSample("ReviewSummaryToolCard"),
    icon: SearchCheck,
    render: (options) => <ConcreteProminentPreview {...options} kind="review" />,
    section: "tool-card",
    specimens: [{ tool: "ReviewSessionSummary" }],
  },
  RunCodeToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("RunCodeToolCard"),
    icon: Code2,
    render: (options) => <StandardAmbientPreview {...options} kind="run-code" />,
    section: "tool-card",
    specimens: [{ tool: "RunCode" }],
  },
  SessionControlToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("SessionControlToolCard"),
    icon: Layers,
    render: (options) => <SessionPreview {...options} kind="control" />,
    section: "tool-card",
    specimens: [{ tool: "SessionControl" }],
  },
  SessionMessageToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("SessionMessageToolCard"),
    icon: MessageSquare,
    render: (options) => <SessionPreview {...options} kind="message" />,
    section: "tool-card",
    specimens: [{ tool: "SessionMessage" }],
  },
  SkillToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("SkillToolCard"),
    icon: Zap,
    render: (options) => <ActivityPreview {...options} kind="skill" />,
    section: "tool-card",
    specimens: [{ tool: "Skill" }],
  },
  TerminalControlToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("TerminalControlToolCard"),
    icon: SquareTerminal,
    render: (options) => <ActivityPreview {...options} kind="terminal-control" />,
    section: "tool-card",
    specimens: [{ tool: "TerminalControl" }],
  },
  TodoToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("TodoToolCard"),
    icon: ListTodo,
    render: (options) => <StandardAmbientPreview {...options} kind="todo" />,
    section: "tool-card",
    specimens: [{ tool: "TodoWrite" }],
  },
  ViewImageToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("ViewImageToolCard"),
    icon: ImageIcon,
    render: (options) => <StandardAmbientPreview {...options} kind="view-image" />,
    section: "tool-card",
    specimens: [{ tool: "view_image" }],
  },
  WebFetchToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("WebFetchToolCard"),
    icon: Globe,
    render: (options) => <StandardAmbientPreview {...options} kind="web-fetch" />,
    section: "tool-card",
    specimens: [{ tool: "WebFetch" }],
  },
  WebSearchToolCard: {
    attention: "ambient",
    codeSample: concreteCodeSample("WebSearchToolCard"),
    icon: Search,
    render: (options) => <SearchPreview {...options} kind="web" />,
    section: "tool-card",
    specimens: [{ tool: "WebSearch" }],
  },
} as const satisfies FlowChatPreviewDefinitionMap;

function isFlowChatComponent(
  component: RegisteredComponent,
): component is FlowChatComponentMeta {
  return component.category === "flow-chat";
}

export const flowChatPreviewRegistry = componentRegistry
  .filter(isFlowChatComponent)
  .map((component) => ({
    component,
    definition: flowChatPreviewDefinitions[component.name],
  }));

export function getFlowChatPreviewDefinition(
  componentName: string,
): FlowChatPreviewDefinition | undefined {
  if (!Object.prototype.hasOwnProperty.call(flowChatPreviewDefinitions, componentName)) {
    return undefined;
  }

  return flowChatPreviewDefinitions[componentName as FlowChatComponentName];
}

export function FlowChatComponentPreview({
  componentName,
  interactive = true,
  specimen,
  state,
}: {
  componentName: string;
  interactive?: boolean;
  specimen?: FlowChatToolSpecimen;
  state?: string;
}) {
  const entry = flowChatPreviewRegistry.find(
    ({ component }) => component.name === componentName,
  );
  const definition = getFlowChatPreviewDefinition(componentName);

  if (!entry || !definition) {
    return null;
  }

  const resolvedState = (state ?? entry.component.states[0]) as FlowChatPreviewState;

  return (
    <Fragment key={`${componentName}-${resolvedState}-${specimen?.tool ?? "component"}`}>
      {definition.render({
        interactive,
        specimen,
        state: resolvedState,
      })}
    </Fragment>
  );
}
