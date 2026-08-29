import assert from "node:assert/strict";
import test from "node:test";
import { componentRegistry } from "../dist/registry.js";

test("component names remain unique", () => {
  const names = componentRegistry.map((component) => component.name);
  assert.equal(new Set(names).size, names.length);
});

test("registry exposes only the formal stable components", () => {
  assert.deepEqual(
    componentRegistry.map((component) => component.name),
    [
      "ActionCard",
      "ActionItem",
      "ActivityItem",
      "AgentControlToolCard",
      "AgentWaitToolCard",
      "AmbientToolCard",
      "AskUser",
      "Button",
      "Card",
      "ChatComposer",
      "CommandToolCard",
      "Composer",
      "ConfirmDialog",
      "ContextCompressionToolCard",
      "DefaultToolCard",
      "DirectoryListToolCard",
      "Field",
      "FieldGroup",
      "FileDiffToolCard",
      "FileOperationToolCard",
      "GetToolSpecToolCard",
      "GitToolCard",
      "GlobSearchToolCard",
      "GrepSearchToolCard",
      "Icon",
      "IconButton",
      "Input",
      "KeyHint",
      "Menu",
      "Modal",
      "NavigationPanel",
      "PageDeployToolCard",
      "PageHeader",
      "PagePublishToolCard",
      "ProminentToolCard",
      "ReadFileToolCard",
      "ReviewSummaryToolCard",
      "RunCodeToolCard",
      "ScrollArea",
      "SearchField",
      "SegmentedControl",
      "Select",
      "SessionControlToolCard",
      "SessionMessageToolCard",
      "SkillToolCard",
      "StatusPill",
      "Switch",
      "TabGroup",
      "TerminalControlToolCard",
      "TodoToolCard",
      "Toolbar",
      "Tooltip",
      "ViewImageToolCard",
      "WebFetchToolCard",
      "WebSearchToolCard",
    ],
  );
  assert.equal(
    componentRegistry.every((component) => component.maturity === "stable"),
    true,
  );
});

test("every registered component declares states and owned tokens", () => {
  for (const component of componentRegistry) {
    assert.ok(component.states.length > 0, `${component.name} has no declared states.`);
    assert.ok(component.tokens.length > 0, `${component.name} has no declared tokens.`);
    assert.equal(
      component.tokens.every(
        (token) =>
          token.startsWith("border.") ||
          token.startsWith("color.") ||
          token.startsWith("control.") ||
          token.startsWith("font.") ||
          token.startsWith("lineHeight.") ||
          token.startsWith("layout.") ||
          token.startsWith("lineHeight.") ||
          token.startsWith("overlay.") ||
          token.startsWith("radius.") ||
          token.startsWith("scrollbar.") ||
          token.startsWith("shadow.") ||
          token.startsWith("space."),
      ),
      true,
      `${component.name} contains a token outside the allowed public layers.`,
    );
  }
});
