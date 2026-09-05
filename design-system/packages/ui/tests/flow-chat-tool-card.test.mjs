import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  AgentControlToolCard,
  AgentWaitToolCard,
  AmbientToolCard,
  AmbientToolCardHeader,
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
  ProminentToolCardSummary,
  ReadFileToolCard,
  ReviewSummaryToolCard,
  RunCodeToolCard,
  SessionControlToolCard,
  SessionMessageToolCard,
  SkillToolCard,
  TerminalControlToolCard,
  TodoToolCard,
  ToolCardChangeSummary,
  ToolCardActions,
  ToolCardCopyButton,
  ViewImageToolCard,
  WebFetchToolCard,
  WebSearchToolCard,
} from "../dist/flow-chat.js";

test("flow-chat entry publishes the ambient and prominent framework anatomy", () => {
  const ambientMarkup = renderToStaticMarkup(
    createElement(AmbientToolCard, {
      expandedContent: createElement("div", null, "Supporting detail"),
      header: createElement(AmbientToolCardHeader, {
        action: "Read file",
        content: "src/index.ts",
        icon: createElement("svg", { "data-icon": "file" }),
      }),
      onClick() {},
      status: "completed",
    }),
  );
  const prominentMarkup = renderToStaticMarkup(
    createElement(ProminentToolCard, {
      expandedContent: createElement("div", null, "Command output"),
      summary: createElement(ProminentToolCardSummary, {
        action: "Run command",
        actions: createElement(
          ToolCardActions,
          null,
          createElement(ToolCardCopyButton, { label: "Copy", onPress() {} }),
        ),
        content: createElement("code", null, "pnpm test"),
        extra: createElement(ToolCardChangeSummary, {
          additions: 6,
          "aria-label": "6 additions and 0 deletions",
          deletions: 0,
        }),
        icon: createElement("svg", { "data-icon": "terminal" }),
      }),
      onToggle() {},
      status: "completed",
    }),
  );

  assert.match(ambientMarkup, /data-openbitfun-attention="ambient"/);
  assert.match(ambientMarkup, /data-openbitfun-part="surface"/);
  assert.match(ambientMarkup, /data-openbitfun-part="iconAffordanceButton"/);
  assert.match(ambientMarkup, /aria-label="Expand details"/);
  assert.match(prominentMarkup, /data-openbitfun-attention="prominent"/);
  assert.match(prominentMarkup, /data-openbitfun-part="summary"/);
  assert.doesNotMatch(prominentMarkup, /data-openbitfun-part="header"/);
  assert.match(prominentMarkup, /data-openbitfun-part="extra"/);
  assert.match(prominentMarkup, /data-openbitfun-part="changeSummary"/);
  assert.match(prominentMarkup, /data-openbitfun-change="added">\+6/);
  assert.match(prominentMarkup, /data-openbitfun-change="removed">-0/);
  assert.match(prominentMarkup, /data-openbitfun-part="actionRegion"/);
  assert.match(prominentMarkup, /data-openbitfun-part="actions"/);
  assert.match(prominentMarkup, /data-openbitfun-part="copyButton"/);
  assert.match(prominentMarkup, /data-openbitfun-part="affordanceButton"/);
  assert.match(prominentMarkup, /aria-expanded="false"/);
});

test("tool-card pointer cursors are limited to interactive surfaces and buttons", async () => {
  const styles = await readFile(
    new URL("../src/flow-chat/tool-cards/FlowChatToolCard.module.css", import.meta.url),
    "utf8",
  );
  const iconButtonStyles = await readFile(
    new URL("../src/components/IconButton/IconButton.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /\.prominentSurface\s*\{[^}]*cursor:\s*default;/s);
  assert.match(styles, /\.prominentSurface\[data-openbitfun-interactive="true"\]\s*\{\s*cursor:\s*pointer;/s);
  assert.match(styles, /\.ambientSurface\s*\{[^}]*cursor:\s*default;/s);
  assert.match(styles, /\.ambientSurface\[data-openbitfun-interactive="true"\]\s*\{\s*cursor:\s*pointer;/s);
  assert.match(iconButtonStyles, /\.button\s*\{[^}]*cursor:\s*pointer;/s);
});

test("tool-card summary surfaces prevent accidental text selection without locking details", async () => {
  const styles = await readFile(
    new URL("../src/flow-chat/tool-cards/FlowChatToolCard.module.css", import.meta.url),
    "utf8",
  );
  const surfaceRule = styles.match(/\.surface\s*\{([^}]*)\}/s)?.[1];
  const collapseRule = styles.match(/\.collapse\s*\{([^}]*)\}/s)?.[1];

  assert.ok(surfaceRule);
  assert.ok(collapseRule);
  assert.match(surfaceRule, /-webkit-user-select:\s*none/);
  assert.match(surfaceRule, /user-select:\s*none/);
  assert.doesNotMatch(collapseRule, /user-select/);
});

test("tool-card change summaries use dedicated code-change colors", async () => {
  const styles = await readFile(
    new URL("../src/flow-chat/tool-cards/FlowChatToolCard.module.css", import.meta.url),
    "utf8",
  );

  assert.match(
    styles,
    /\.changeSummary \[data-openbitfun-change="added"\]\s*\{\s*color: var\(--openbitfun-color-code-change-added\);/,
  );
  assert.match(
    styles,
    /\.changeSummary \[data-openbitfun-change="removed"\]\s*\{\s*color: var\(--openbitfun-color-code-change-removed\);/,
  );
});

test("prominent error status opens error content without a separate failure flag", () => {
  const markup = renderToStaticMarkup(
    createElement(ProminentToolCard, {
      errorContent: createElement("div", null, "Command failed"),
      summary: createElement(ProminentToolCardSummary, { action: "Run command" }),
      status: "error",
    }),
  );

  assert.match(markup, /data-openbitfun-state="failed"/);
  assert.match(markup, /data-openbitfun-part="errorCollapse"[^>]+data-open="true"/);
  assert.match(markup, /data-openbitfun-part="error"/);
  assert.match(markup, /Command failed/);
});

test("prominent error status can opt into expandable supporting details", () => {
  const markup = renderToStaticMarkup(
    createElement(ProminentToolCard, {
      allowExpandedWhenFailed: true,
      errorContent: createElement("div", null, "Command failed"),
      expandedContent: createElement("div", null, "Invocation input"),
      summary: createElement(ProminentToolCardSummary, { action: "Run command" }),
      isExpanded: true,
      onToggle() {},
      status: "error",
    }),
  );

  assert.match(markup, /data-openbitfun-state="expanded failed"/);
  assert.match(markup, /data-openbitfun-expandable="true"/);
  assert.match(markup, /aria-expanded="true"/);
  assert.match(markup, /data-openbitfun-part="expandedCollapse"[^>]+data-open="true"/);
  assert.match(markup, /Invocation input/);
  assert.match(markup, /data-openbitfun-part="errorCollapse"[^>]+data-open="true"/);
  assert.match(markup, /Command failed/);
});

test("cancelled and rejected tool cards rely on status copy instead of a duplicate x glyph", () => {
  for (const [status, statusLabel] of [
    ["cancelled", "Cancelled"],
    ["rejected", "Rejected"],
  ]) {
    const agentMarkup = renderToStaticMarkup(
      createElement(AgentControlToolCard, {
        agentName: "reviewer",
        status,
        statusLabel,
      }),
    );

    assert.match(agentMarkup, new RegExp(`data-openbitfun-part="agentStatus"[^>]*>${statusLabel}<`));
    assert.doesNotMatch(agentMarkup, /data-openbitfun-part="status"/);
    assert.doesNotMatch(agentMarkup, /lucide-x/);
  }

  const ambientMarkup = renderToStaticMarkup(
    createElement(DefaultToolCard, {
      displayName: "Custom tool",
      icon: createElement("svg", { "data-icon": "custom-tool" }),
      status: "cancelled",
      summary: "Cancelled",
      toolName: "custom_tool",
    }),
  );

  assert.match(ambientMarkup, /data-openbitfun-part="statusSlot"[^>]+data-default-icon="tool"/);
  assert.match(ambientMarkup, /data-openbitfun-part="toolIconLayer"/);
  assert.match(ambientMarkup, /data-icon="custom-tool"/);
  assert.doesNotMatch(ambientMarkup, /data-openbitfun-part="statusLayer"|lucide-x/);
});

test("file-operation failures stay collapsed and use the warning emphasis status icon", async () => {
  const createFailedCard = (isExpanded) => renderToStaticMarkup(
    createElement(FileOperationToolCard, {
      actionLabel: "Edit failed",
      error: {
        message: "The target text was not found.",
        title: "Detailed failure",
      },
      isExpanded,
      onToggle() {},
      operation: "edit",
      path: "src/index.ts",
      pathLabel: "src/index.ts",
      status: "error",
    }),
  );
  const collapsedMarkup = createFailedCard(false);
  const expandedMarkup = createFailedCard(true);
  const styles = await readFile(
    new URL("../src/flow-chat/tool-cards/FileOperationToolCard.module.css", import.meta.url),
    "utf8",
  );

  assert.match(collapsedMarkup, /data-openbitfun-state="failed"/);
  assert.match(collapsedMarkup, /data-openbitfun-expandable="true"/);
  assert.match(collapsedMarkup, /aria-expanded="false"/);
  assert.match(collapsedMarkup, /data-openbitfun-icon="warning"/);
  assert.match(collapsedMarkup, /data-openbitfun-part="errorCollapse"[^>]+data-open="false"/);
  assert.match(collapsedMarkup, /src\/index\.ts/);
  assert.doesNotMatch(collapsedMarkup, /Detailed failure/);
  assert.doesNotMatch(collapsedMarkup, /The target text was not found\./);
  assert.match(expandedMarkup, /data-openbitfun-state="expanded failed"/);
  assert.match(expandedMarkup, /aria-expanded="true"/);
  assert.match(expandedMarkup, /data-openbitfun-part="errorCollapse"[^>]+data-open="true"/);
  assert.match(expandedMarkup, /Detailed failure/);
  assert.match(expandedMarkup, /The target text was not found\./);
  assert.match(
    styles,
    /\.warningStatusIcon\s*\{\s*color: var\(--openbitfun-color-status-warning-emphasis\);/,
  );
  assert.match(
    styles,
    /\.errorTitle > :where\(svg, img\)\s*\{[^}]*inline-size: var\(--openbitfun-font-size-xl\);[^}]*block-size: var\(--openbitfun-font-size-xl\);/s,
  );
});

test("prominent actions stay hidden until hover, preview-hover, or keyboard focus", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /data-openbitfun-preview-state=hover/);
  assert.match(styles, /:focus-within/);
  assert.match(styles, /max-inline-size:\s*0/);
  assert.match(styles, /opacity:\s*0/);
  assert.match(styles, /pointer-events:\s*none/);
  assert.match(styles, /margin-inline-start:\s*auto/);
  assert.match(styles, /font-variant-numeric:\s*proportional-nums/);
  assert.match(styles, /--openbitfun-control-height-sm/);
  assert.match(styles, /--openbitfun-space-6/);
  assert.match(styles, /--openbitfun-radius-md/);
  assert.match(styles, /--openbitfun-color-focus-ring/);
});

test("FlowChat tool-card shells stay flat at rest and on hover", async () => {
  const styles = await readFile(
    new URL("../src/flow-chat/tool-cards/FlowChatToolCard.module.css", import.meta.url),
    "utf8",
  );
  const prominentRule = styles.match(/\.prominentRoot\s*\{([^}]*)\}/s)?.[1];
  const ambientExpandedRule = styles.match(/\.ambientExpandedShell\s*\{([^}]*)\}/s)?.[1];
  const ambientOutlineRule = styles.match(/\.ambientRoot::after\s*\{([^}]*)\}/s)?.[1];

  assert.ok(prominentRule);
  assert.ok(ambientExpandedRule);
  assert.ok(ambientOutlineRule);
  assert.match(prominentRule, /box-shadow:\s*none/);
  assert.match(ambientExpandedRule, /box-shadow:\s*none/);
  assert.match(ambientOutlineRule, /position:\s*absolute/);
  assert.match(ambientOutlineRule, /pointer-events:\s*none/);
  assert.doesNotMatch(styles, /box-shadow:\s*var\(--openbitfun-shadow-(?:xs|sm)\)/);
  assert.doesNotMatch(styles, /box-shadow\s+var\(--_tool-card-transition\)/);
});

test("ambient tool-card summary geometry stays stable while details expand", async () => {
  const styles = await readFile(
    new URL("../src/flow-chat/tool-cards/FlowChatToolCard.module.css", import.meta.url),
    "utf8",
  );
  const ambientSurfaceRule = styles.match(/\.ambientSurface\s*\{([^}]*)\}/s)?.[1];
  const expandedAmbientSurfaceRule = styles.match(
    /\.ambientExpandedShell \.ambientSurface\s*\{([^}]*)\}/s,
  )?.[1];

  assert.ok(ambientSurfaceRule);
  assert.ok(expandedAmbientSurfaceRule);
  assert.match(
    ambientSurfaceRule,
    /min-block-size:\s*max\(1lh,\s*var\(--openbitfun-control-tool-card-ambient-row-min-block-size\)\)/,
  );
  assert.doesNotMatch(ambientSurfaceRule, /--openbitfun-control-height-sm/);
  assert.doesNotMatch(expandedAmbientSurfaceRule, /min-block-size|padding/);
});

test("ambient tool-card collapse has no delayed shell state or layout-changing shell chrome", async () => {
  const source = await readFile(
    new URL("../src/flow-chat/tool-cards/FlowChatToolCard.tsx", import.meta.url),
    "utf8",
  );
  const styles = await readFile(
    new URL("../src/flow-chat/tool-cards/FlowChatToolCard.module.css", import.meta.url),
    "utf8",
  );
  const ambientExpandedRule = styles.match(/\.ambientExpandedShell\s*\{([^}]*)\}/s)?.[1];

  assert.ok(ambientExpandedRule);
  assert.match(source, /const expandedShell = Boolean\(isExpanded && hasExpandedContent\)/);
  assert.doesNotMatch(source, /keepExpandedShell|collapseTimerRef/);
  assert.doesNotMatch(ambientExpandedRule, /margin|border(?!-color)/);
});

test("FlowChat tool-card summary rows share compact title and content typography", async () => {
  const styles = await readFile(
    new URL("../src/flow-chat/tool-cards/FlowChatToolCard.module.css", import.meta.url),
    "utf8",
  );
  const commandStyles = await readFile(
    new URL("../src/flow-chat/tool-cards/CommandToolCard.module.css", import.meta.url),
    "utf8",
  );
  const fileOperationStyles = await readFile(
    new URL("../src/flow-chat/tool-cards/FileOperationToolCard.module.css", import.meta.url),
    "utf8",
  );
  const rootRule = styles.match(/\.prominentRoot,\s*\.ambientRoot\s*\{([^}]*)\}/s)?.[1];
  const prominentSizeRule = styles.match(/\.actionLabel,\s*\.content,\s*\.extra\s*\{([^}]*)\}/s)?.[1];
  const ambientSizeRule = styles.match(/\.ambientAction,\s*\.ambientContent,\s*\.ambientExtra\s*\{([^}]*)\}/s)?.[1];
  const directChildRule = styles.match(/\.actionLabel > \*,[\s\S]*?\.ambientExtra > \*\s*\{([^}]*)\}/s)?.[1];
  const prominentTitleRule = styles.match(/\.actionLabel\s*\{([^}]*)\}/s)?.[1];
  const prominentContentRule = styles.match(/\.content\s*\{([^}]*)\}/s)?.[1];
  const ambientTitleRule = styles.match(/\.ambientAction\s*\{([^}]*)\}/s)?.[1];
  const ambientContentRule = styles.match(/\.ambientContent\s*\{([^}]*)\}/s)?.[1];
  const changeSummaryRule = styles.match(/\.changeSummary\s*\{([^}]*)\}/s)?.[1];
  const commandRule = commandStyles.match(/\.command\s*\{([^}]*)\}/s)?.[1];
  const filePathRule = fileOperationStyles.match(/\.path\s*\{([^}]*)\}/s)?.[1];

  assert.ok(rootRule);
  assert.ok(prominentSizeRule);
  assert.ok(ambientSizeRule);
  assert.ok(directChildRule);
  assert.ok(prominentTitleRule);
  assert.ok(prominentContentRule);
  assert.ok(ambientTitleRule);
  assert.ok(ambientContentRule);
  assert.ok(changeSummaryRule);
  assert.ok(commandRule);
  assert.ok(filePathRule);
  assert.match(rootRule, /--_tool-card-font-family:\s*var\(--openbitfun-type-body-sm-font-family\)/);
  assert.match(rootRule, /--_tool-card-font-size:\s*var\(--openbitfun-type-body-sm-font-size\)/);
  assert.match(rootRule, /--_tool-card-title-font-weight:\s*var\(--openbitfun-type-label-lg-font-weight\)/);
  assert.match(rootRule, /--_tool-card-content-font-weight:\s*var\(--openbitfun-type-body-sm-font-weight\)/);
  assert.match(rootRule, /font-variant-numeric:\s*proportional-nums/);
  assert.match(prominentSizeRule, /font-size:\s*var\(--_tool-card-font-size\)/);
  assert.match(ambientSizeRule, /font-size:\s*var\(--_tool-card-font-size\)/);
  assert.match(directChildRule, /font:\s*inherit/);
  assert.match(prominentTitleRule, /font-weight:\s*var\(--_tool-card-title-font-weight\)/);
  assert.match(ambientTitleRule, /font-weight:\s*var\(--_tool-card-title-font-weight\)/);
  assert.match(prominentContentRule, /font-weight:\s*var\(--_tool-card-content-font-weight\)/);
  assert.match(ambientContentRule, /font-weight:\s*var\(--_tool-card-content-font-weight\)/);
  assert.match(changeSummaryRule, /font-family:\s*var\(--_tool-card-font-family\)/);
  assert.match(changeSummaryRule, /font-weight:\s*var\(--_tool-card-content-font-weight\)/);
  assert.doesNotMatch(commandRule, /font-(?:family|size|weight):/);
  assert.doesNotMatch(filePathRule, /font-(?:family|size|weight):/);
});

test("tool cards reserve monospace for file-edit code previews", async () => {
  const directory = new URL("../src/flow-chat/tool-cards/", import.meta.url);
  const files = (await readdir(directory)).filter((file) => file.endsWith(".module.css"));
  const allowedMonoUses = new Map([
    ["ProminentToolCards.module.css", 1],
  ]);

  for (const file of files) {
    const stylesheet = await readFile(new URL(file, directory), "utf8");
    const monoUses = stylesheet.match(/--openbitfun-type-code-md-font-family/g)?.length ?? 0;
    assert.equal(monoUses, allowedMonoUses.get(file) ?? 0, file);
    assert.doesNotMatch(stylesheet, /font-variant-numeric:\s*tabular-nums/, file);
  }

  const previewStyles = await readFile(
    new URL("../../../apps/design-lab/src/preview/FlowChatPreviewRegistry.css", import.meta.url),
    "utf8",
  );
  assert.equal(previewStyles.match(/--openbitfun-type-code-md-font-family/g)?.length ?? 0, 1);
  assert.doesNotMatch(previewStyles, /font-variant-numeric:\s*tabular-nums/);
  assert.match(
    previewStyles,
    /\.flow-chat-tool-card-preview__diff\s*\{[^}]*font-family:\s*var\(--openbitfun-type-code-md-font-family\)/s,
  );
});

test("command text remains plain when a host applies inline-code chrome", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");
  const commandRule = styles.match(/\.[_a-zA-Z0-9-]*command[_a-zA-Z0-9-]*\{([^}]*)\}/)?.[1];

  assert.ok(commandRule);
  assert.match(commandRule, /margin:\s*0/);
  assert.match(commandRule, /padding:\s*0/);
  assert.match(commandRule, /border:\s*0/);
  assert.match(commandRule, /border-radius:\s*0/);
  assert.match(commandRule, /background:\s*transparent/);
});

test("ambient card cursors distinguish static traces from interactive cards", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");
  const staticMarkup = renderToStaticMarkup(
    createElement(AgentWaitToolCard, {
      action: "Wait for agents",
      status: "completed",
      summary: "All agents completed",
    }),
  );

  assert.match(
    styles,
    /ambientSurface[^}]*\{[^}]*cursor:\s*default/s,
  );
  assert.match(
    styles,
    /ambientSurface[^}]*\[data-openbitfun-interactive=["']?true["']?\][^{]*\{[^}]*cursor:\s*pointer/s,
  );
  assert.match(staticMarkup, /data-openbitfun-interactive="false"/);
  assert.doesNotMatch(staticMarkup, /role="button"/);
  assert.doesNotMatch(staticMarkup, /tabindex="0"/);
});

test("standard FlowChat tool views publish their concrete component contracts", () => {
  const readMarkup = renderToStaticMarkup(
    createElement(ReadFileToolCard, {
      action: "Read file:",
      accessibleLabel: "Open src/index.ts",
      content: "src/index.ts · 128 lines",
      interactive: true,
      onOpen() {},
      status: "completed",
    }),
  );
  const contextMarkup = renderToStaticMarkup(
    createElement(ContextCompressionToolCard, {
      status: "completed",
      summary: "Compressed context length 31k (compression ratio 75%)",
      title: "Compress context",
    }),
  );
  const commandMarkup = renderToStaticMarkup(
    createElement(CommandToolCard, {
      action: "Run command",
      command: "pnpm test",
      emptyCommand: "No command",
      footerItems: [{ label: "Exit code", value: "0" }],
      isExpanded: true,
      output: createElement("pre", null, "57 tests passed"),
      status: "completed",
    }),
  );
  const deleteMarkup = renderToStaticMarkup(
    createElement(FileOperationToolCard, {
      actionLabel: "Delete file",
      operation: "delete",
      path: "dist/stale.js",
      pathLabel: "dist/stale.js",
      status: "completed",
    }),
  );
  const editMarkup = renderToStaticMarkup(
    createElement(FileOperationToolCard, {
      actionLabel: "Edit file",
      changeSummary: {
        additions: 6,
        deletions: 0,
        label: "6 additions and 0 deletions",
      },
      isExpanded: true,
      onOpenFile: {
        label: "Open file",
        onPress() {},
        testId: "open-file-action",
      },
      onToggle() {},
      operation: "edit",
      path: "src/index.ts",
      pathLabel: "src/index.ts",
      preview: createElement("pre", null, "+ migrated view"),
      status: "completed",
    }),
  );

  assert.match(readMarkup, /data-openbitfun-tool-card="read-file"/);
  assert.match(readMarkup, /data-openbitfun-attention="ambient"/);
  assert.match(readMarkup, /data-openbitfun-direct-action="true"/);
  assert.match(readMarkup, /role="button"/);
  assert.match(readMarkup, /tabindex="0"/);
  assert.match(readMarkup, /aria-label="Open src\/index\.ts"/);
  assert.match(contextMarkup, /data-openbitfun-component="context-compression-tool-card"/);
  assert.match(contextMarkup, /data-openbitfun-part="summary"/);
  assert.match(contextMarkup, /Compressed context length 31k \(compression ratio 75%\)/);
  assert.doesNotMatch(contextMarkup, /data-openbitfun-part="(?:savings|meta|tokenChange)"/);
  assert.match(commandMarkup, /data-openbitfun-component="command-tool-card"/);
  assert.match(commandMarkup, /data-openbitfun-part="outputFrame"/);
  assert.match(commandMarkup, /57 tests passed/);
  assert.match(deleteMarkup, /data-openbitfun-operation="delete"/);
  assert.match(deleteMarkup, /data-openbitfun-attention="ambient"/);
  assert.match(deleteMarkup, /data-openbitfun-part="action">Delete file<\/span>/);
  assert.match(deleteMarkup, /data-openbitfun-part="content">/);
  assert.match(editMarkup, /data-openbitfun-operation="edit"/);
  assert.match(editMarkup, /data-openbitfun-attention="prominent"/);
  assert.match(editMarkup, /\+ migrated view/);
  assert.match(editMarkup, /data-openbitfun-part="changeSummary"/);
  assert.match(editMarkup, /data-openbitfun-part="affordanceButton"/);
  assert.match(editMarkup, /data-openbitfun-part="trailingActions"[^>]+data-divider="true"/);
  assert.match(editMarkup, /data-openbitfun-part="openPanelButton"/);
  assert.match(editMarkup, /data-openbitfun-affordance="open-panel-right"/);
  assert.match(editMarkup, /data-openbitfun-icon="open-panel-right"/);
  assert.equal((editMarkup.match(/<button\b/g) ?? []).length, 2);
  assert.ok(
    editMarkup.indexOf('data-openbitfun-part="affordanceButton"')
      < editMarkup.indexOf('data-openbitfun-part="trailingActions"'),
  );
  assert.doesNotMatch(editMarkup, /lucide-file-diff|lucide-chevron-right/);
});

test("every migrated FlowChat tool view publishes a stable concrete card identity", () => {
  const cards = [
    ["agent-control", createElement(AgentControlToolCard, {
      agentName: "reviewer",
      onToggle() {},
      prompt: "Review the migration",
      status: "running",
      statusLabel: "Running",
    })],
    ["agent-wait", createElement(AgentWaitToolCard, {
      action: "Wait for agents",
      status: "running",
      summary: "Waiting",
    })],
    ["default", createElement(DefaultToolCard, {
      displayName: "Custom tool",
      status: "completed",
      summary: "Completed",
      toolName: "custom_tool",
    })],
    ["directory-list", createElement(DirectoryListToolCard, {
      results: [{ key: "src", title: "src/" }],
      status: "completed",
      summary: "18 entries",
    })],
    ["file-diff", createElement(FileDiffToolCard, {
      action: "Get diff",
      changeSummary: {
        additions: 12,
        deletions: 0,
        label: "12 additions and 0 deletions",
      },
      path: "src/index.ts",
      pathLabel: "index.ts",
      status: "completed",
    })],
    ["get-tool-spec", createElement(GetToolSpecToolCard, {
      action: "Read tool spec",
      status: "completed",
      summary: "Loaded",
    })],
    ["git", createElement(GitToolCard, {
      action: "Git",
      command: "git status",
      status: "completed",
    })],
    ["glob-search", createElement(GlobSearchToolCard, {
      status: "completed",
      summary: "42 files",
    })],
    ["grep-search", createElement(GrepSearchToolCard, {
      resultText: "src/index.ts:12",
      status: "completed",
      summary: "1 match",
    })],
    ["page-deploy", createElement(PageDeployToolCard, {
      action: "Deploy page",
      status: "completed",
      subject: "docs",
    })],
    ["page-publish", createElement(PagePublishToolCard, {
      action: "Publish page",
      status: "completed",
      subject: "docs",
    })],
    ["review-summary", createElement(ReviewSummaryToolCard, {
      status: "completed",
      summary: "No blocking issues",
      title: "Review complete",
    })],
    ["run-code", createElement(RunCodeToolCard, {
      action: "Run code",
      status: "completed",
      summary: "Verified",
    })],
    ["session-control", createElement(SessionControlToolCard, {
      action: "Session control",
      status: "completed",
      summary: "Created session",
    })],
    ["session-message", createElement(SessionMessageToolCard, {
      action: "Session message",
      status: "completed",
      summary: "Sent message",
    })],
    ["skill", createElement(SkillToolCard, {
      action: "Skill",
      status: "completed",
      summary: "Loaded",
    })],
    ["terminal-control", createElement(TerminalControlToolCard, {
      action: "Terminal control",
      status: "completed",
      summary: "Interrupted",
    })],
    ["todo", createElement(TodoToolCard, {
      completedCount: 1,
      items: [{ content: "Migrate card", key: "one", status: "completed" }],
      status: "completed",
      title: "Tasks",
      totalCount: 1,
    })],
    ["view-image", createElement(ViewImageToolCard, {
      alt: "Preview",
      source: "data:image/png;base64,AAAA",
      status: "completed",
      statusText: "Viewed image",
    })],
    ["web-fetch", createElement(WebFetchToolCard, {
      status: "completed",
      title: "Fetched page",
    })],
    ["web-search", createElement(WebSearchToolCard, {
      status: "completed",
      summary: "3 results",
    })],
  ];

  for (const [identity, card] of cards) {
    const markup = renderToStaticMarkup(card);
    assert.match(markup, new RegExp(`data-openbitfun-tool-card="${identity}"`), identity);
    assert.match(markup, /data-openbitfun-component="flow-chat-tool-card"/, identity);
    assert.match(markup, /data-openbitfun-status="(?:completed|running)"/, identity);
  }
});

test("file diff summary matches the compact file-operation information hierarchy", async () => {
  const markup = renderToStaticMarkup(createElement(FileDiffToolCard, {
    action: "Get diff:",
    changeSummary: {
      additions: 12,
      deletions: 0,
      label: "12 additions and 0 deletions",
    },
    path: "src/index.ts",
    pathLabel: "index.ts",
    status: "completed",
  }));
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");
  const pathRule = styles.match(/\.[_a-zA-Z0-9-]*diffPath[_a-zA-Z0-9-]*\{([^}]*)\}/)?.[1];

  assert.match(markup, /data-path="src\/index\.ts"[^>]*data-openbitfun-part="path"[^>]*title="src\/index\.ts"/);
  assert.match(markup, />index\.ts<\/span>/);
  assert.match(markup, /data-openbitfun-part="changeSummary"/);
  assert.match(markup, /aria-label="12 additions and 0 deletions"/);
  assert.match(markup, /data-openbitfun-change="added">\+12/);
  assert.match(markup, /data-openbitfun-change="removed">-0/);
  assert.doesNotMatch(markup, /Git HEAD|data-openbitfun-part="diffType"/);
  assert.ok(pathRule, "file-diff path rule should exist");
  assert.match(pathRule, /var\(--openbitfun-color-content-secondary\)/);
  assert.doesNotMatch(pathRule, /font-(?:family|size|weight):/);
});

test("concrete tool views expose semantic parts instead of legacy CSS selectors", () => {
  const agentMarkup = renderToStaticMarkup(createElement(AgentControlToolCard, {
    agentName: "reviewer",
    agentModel: "gpt-5.6",
    interruptAction: {
      label: "Stop agent",
      onPress() {},
    },
    onOpenAgent() {},
    openAgentLabel: "Open agent",
    onToggle() {},
    prompt: "Review the migration",
    status: "running",
    statusMeta: "4s",
    statusLabel: "Running",
    summary: "Review the shared FlowChat boundary",
  }));
  const fetchMarkup = renderToStaticMarkup(createElement(WebFetchToolCard, {
    details: ["markdown"],
    isExpanded: true,
    onOpenUrl() {},
    openUrlLabel: "Open source",
    status: "completed",
    title: "Architecture",
    url: "https://openbitfun.com/docs",
  }));
  const imageMarkup = renderToStaticMarkup(createElement(ViewImageToolCard, {
    alt: "Preview",
    isExpanded: true,
    onOpenPreview() {},
    source: "data:image/png;base64,AAAA",
    status: "completed",
    statusText: "Viewed image",
  }));

  assert.match(agentMarkup, /data-openbitfun-part="agentIdentity"/);
  assert.match(agentMarkup, /data-openbitfun-part="agentModel"/);
  assert.match(agentMarkup, /data-openbitfun-part="agentSummary"/);
  assert.match(agentMarkup, /data-openbitfun-part="agentMeta"/);
  assert.match(agentMarkup, /data-openbitfun-part="statusSlot"/);
  assert.match(agentMarkup, /data-openbitfun-part="processing"/);
  assert.match(agentMarkup, /data-openbitfun-part="interruptAgentButton"/);
  assert.match(agentMarkup, /data-openbitfun-part="affordanceButton"/);
  assert.match(agentMarkup, /data-openbitfun-part="openAgentButton"/);
  assert.match(agentMarkup, /data-openbitfun-affordance="open-panel-right"/);
  assert.doesNotMatch(agentMarkup, /data-openbitfun-part="expandIndicator"|cube-loading/);
  assert.match(fetchMarkup, /data-openbitfun-part="sourceLink"/);
  assert.match(fetchMarkup, /data-openbitfun-part="detail"/);
  assert.match(imageMarkup, /data-openbitfun-part="imagePreview"/);
});

test("package manifest exposes flow-chat only through built artifacts", async () => {
  const manifest = JSON.parse(
    await readFile(new URL("../package.json", import.meta.url), "utf8"),
  );

  assert.deepEqual(manifest.exports["./flow-chat"], {
    types: "./dist/types/flow-chat.d.ts",
    import: "./dist/flow-chat.js",
  });
});
