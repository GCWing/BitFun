import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
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
      header: createElement(ProminentToolCardHeader, {
        action: "Run command",
        actions: createElement(
          ToolCardHeaderActions,
          null,
          createElement("button", { type: "button" }, "Copy"),
        ),
        content: createElement("code", null, "pnpm test"),
        extra: createElement(ToolCardChangeSummary, {
          additions: 6,
          "aria-label": "6 additions and 0 deletions",
          deletions: 0,
        }),
        icon: createElement("svg", { "data-icon": "terminal" }),
      }),
      onClick() {},
      status: "completed",
    }),
  );

  assert.match(ambientMarkup, /data-bf-attention="ambient"/);
  assert.match(ambientMarkup, /data-bf-part="surface"/);
  assert.match(ambientMarkup, /data-bf-part="iconAffordanceButton"/);
  assert.match(ambientMarkup, /aria-label="Expand details"/);
  assert.match(prominentMarkup, /data-bf-attention="prominent"/);
  assert.match(prominentMarkup, /data-bf-part="extra"/);
  assert.match(prominentMarkup, /data-bf-part="changeSummary"/);
  assert.match(prominentMarkup, /data-bf-change="added">\+6/);
  assert.match(prominentMarkup, /data-bf-change="removed">-0/);
  assert.match(prominentMarkup, /data-bf-part="actionRegion"/);
  assert.match(prominentMarkup, /data-bf-part="actions"/);
  assert.match(prominentMarkup, /data-bf-part="affordanceButton"/);
  assert.match(prominentMarkup, /aria-expanded="false"/);
});

test("prominent error status opens error content without a separate failure flag", () => {
  const markup = renderToStaticMarkup(
    createElement(ProminentToolCard, {
      errorContent: createElement("div", null, "Command failed"),
      header: createElement(ProminentToolCardHeader, { action: "Run command" }),
      status: "error",
    }),
  );

  assert.match(markup, /data-bf-state="failed"/);
  assert.match(markup, /data-bf-part="errorCollapse"[^>]+data-open="true"/);
  assert.match(markup, /data-bf-part="error"/);
  assert.match(markup, /Command failed/);
});

test("prominent error status can opt into expandable supporting details", () => {
  const markup = renderToStaticMarkup(
    createElement(ProminentToolCard, {
      allowExpandedWhenFailed: true,
      errorContent: createElement("div", null, "Command failed"),
      expandedContent: createElement("div", null, "Invocation input"),
      header: createElement(ProminentToolCardHeader, { action: "Run command" }),
      isExpanded: true,
      onClick() {},
      status: "error",
    }),
  );

  assert.match(markup, /data-bf-state="expanded failed"/);
  assert.match(markup, /data-bf-expandable="true"/);
  assert.match(markup, /aria-expanded="true"/);
  assert.match(markup, /data-bf-part="expandedCollapse"[^>]+data-open="true"/);
  assert.match(markup, /Invocation input/);
  assert.match(markup, /data-bf-part="errorCollapse"[^>]+data-open="true"/);
  assert.match(markup, /Command failed/);
});

test("prominent actions stay hidden until hover, preview-hover, or keyboard focus", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /data-bf-preview-state=hover/);
  assert.match(styles, /:focus-within/);
  assert.match(styles, /max-inline-size:\s*0/);
  assert.match(styles, /opacity:\s*0/);
  assert.match(styles, /pointer-events:\s*none/);
  assert.match(styles, /margin-inline-start:\s*auto/);
  assert.match(styles, /font-variant-numeric:\s*tabular-nums/);
  assert.match(styles, /--bf-control-height-sm/);
  assert.match(styles, /--bf-space-6/);
  assert.match(styles, /--bf-radius-md/);
  assert.match(styles, /--bf-color-focus-ring/);
});

test("FlowChat tool-card shells stay flat at rest and on hover", async () => {
  const styles = await readFile(
    new URL("../src/flow-chat/tool-cards/FlowChatToolCard.module.css", import.meta.url),
    "utf8",
  );
  const prominentRule = styles.match(/\.prominentRoot\s*\{([^}]*)\}/s)?.[1];
  const ambientExpandedRule = styles.match(/\.ambientExpandedShell\s*\{([^}]*)\}/s)?.[1];

  assert.ok(prominentRule);
  assert.ok(ambientExpandedRule);
  assert.match(prominentRule, /box-shadow:\s*none/);
  assert.match(ambientExpandedRule, /box-shadow:\s*none/);
  assert.doesNotMatch(styles, /box-shadow:\s*var\(--bf-shadow-(?:xs|sm)\)/);
  assert.doesNotMatch(styles, /box-shadow\s+var\(--_tool-card-transition\)/);
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
    /ambientSurface[^}]*\[data-bf-interactive=["']?true["']?\][^{]*\{[^}]*cursor:\s*pointer/s,
  );
  assert.match(staticMarkup, /data-bf-interactive="false"/);
  assert.doesNotMatch(staticMarkup, /role="button"/);
  assert.doesNotMatch(staticMarkup, /tabindex="0"/);
});

test("standard FlowChat tool views publish their concrete component contracts", () => {
  const readMarkup = renderToStaticMarkup(
    createElement(ReadFileToolCard, {
      accessibleLabel: "Open src/index.ts",
      interactive: true,
      onOpen() {},
      status: "completed",
      summary: "src/index.ts · 128 lines",
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

  assert.match(readMarkup, /data-bf-tool-card="read-file"/);
  assert.match(readMarkup, /data-bf-attention="ambient"/);
  assert.match(readMarkup, /data-bf-direct-action="true"/);
  assert.match(readMarkup, /role="button"/);
  assert.match(readMarkup, /tabindex="0"/);
  assert.match(readMarkup, /aria-label="Open src\/index\.ts"/);
  assert.match(contextMarkup, /data-bf-component="context-compression-tool-card"/);
  assert.match(contextMarkup, /data-bf-part="summary"/);
  assert.match(contextMarkup, /Compressed context length 31k \(compression ratio 75%\)/);
  assert.doesNotMatch(contextMarkup, /data-bf-part="(?:savings|meta|tokenChange)"/);
  assert.match(commandMarkup, /data-bf-component="command-tool-card"/);
  assert.match(commandMarkup, /data-bf-part="outputFrame"/);
  assert.match(commandMarkup, /57 tests passed/);
  assert.match(deleteMarkup, /data-bf-operation="delete"/);
  assert.match(deleteMarkup, /data-bf-attention="ambient"/);
  assert.match(editMarkup, /data-bf-operation="edit"/);
  assert.match(editMarkup, /data-bf-attention="prominent"/);
  assert.match(editMarkup, /\+ migrated view/);
  assert.match(editMarkup, /data-bf-part="changeSummary"/);
  assert.match(editMarkup, /data-bf-part="affordanceButton"/);
  assert.match(editMarkup, /data-bf-part="trailingActions"[^>]+data-divider="true"/);
  assert.match(editMarkup, /data-bf-part="openPanelButton"/);
  assert.match(editMarkup, /data-bf-affordance="open-panel-right"/);
  assert.match(editMarkup, /data-bf-icon="open-panel-right"/);
  assert.equal((editMarkup.match(/<button\b/g) ?? []).length, 2);
  assert.ok(
    editMarkup.indexOf('data-bf-part="affordanceButton"')
      < editMarkup.indexOf('data-bf-part="trailingActions"'),
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
    assert.match(markup, new RegExp(`data-bf-tool-card="${identity}"`), identity);
    assert.match(markup, /data-bf-component="flow-chat-tool-card"/, identity);
    assert.match(markup, /data-bf-status="(?:completed|running)"/, identity);
  }
});

test("file diff header matches the compact file-operation information hierarchy", async () => {
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

  assert.match(markup, /data-path="src\/index\.ts"[^>]*data-bf-part="path"[^>]*title="src\/index\.ts"/);
  assert.match(markup, />index\.ts<\/span>/);
  assert.match(markup, /data-bf-part="changeSummary"/);
  assert.match(markup, /aria-label="12 additions and 0 deletions"/);
  assert.match(markup, /data-bf-change="added">\+12/);
  assert.match(markup, /data-bf-change="removed">-0/);
  assert.doesNotMatch(markup, /Git HEAD|data-bf-part="diffType"/);
  assert.ok(pathRule, "file-diff path rule should exist");
  assert.match(pathRule, /var\(--bf-color-content-secondary\)/);
  assert.match(pathRule, /var\(--bf-font-family-mono\)/);
  assert.match(pathRule, /var\(--bf-font-size-sm\)/);
});

test("concrete tool views expose semantic parts instead of legacy CSS selectors", () => {
  const agentMarkup = renderToStaticMarkup(createElement(AgentControlToolCard, {
    agentName: "reviewer",
    onToggle() {},
    prompt: "Review the migration",
    status: "running",
    statusLabel: "Running",
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

  assert.match(agentMarkup, /data-bf-part="agentIdentity"/);
  assert.match(agentMarkup, /data-bf-part="expandIndicator"/);
  assert.match(fetchMarkup, /data-bf-part="sourceLink"/);
  assert.match(fetchMarkup, /data-bf-part="detail"/);
  assert.match(imageMarkup, /data-bf-part="imagePreview"/);
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
