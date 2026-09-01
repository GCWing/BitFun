import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { SegmentedControl } from "../dist/index.js";

const options = [
  { icon: createElement("svg", { "data-icon": "chat" }), label: "Chat", value: "chat" },
  { label: "Agent", value: "agent" },
];

test("SegmentedControl exposes radiogroup semantics with a roving selected segment", () => {
  const markup = renderToStaticMarkup(
    createElement(SegmentedControl, {
      "aria-label": "Conversation mode",
      defaultValue: "agent",
      options,
    }),
  );

  assert.match(markup, /data-bf-component="segmented-control"/);
  assert.match(markup, /role="radiogroup"/);
  assert.match(markup, /aria-label="Conversation mode"/);
  assert.equal((markup.match(/role="radio"/g) ?? []).length, 2);
  assert.match(markup, /aria-checked="false"[^>]*data-bf-value="chat"/);
  assert.match(markup, /aria-checked="true"[^>]*data-bf-value="agent"/);
  assert.match(markup, /aria-checked="true"[^>]*tabindex="0"/);
  assert.match(markup, /aria-checked="false"[^>]*tabindex="-1"/);
  assert.match(markup, /data-bf-part="icon"/);
  assert.match(markup, /data-bf-part="label">Chat<\/span>/);
});

test("SegmentedControl falls back to the first enabled option when the candidate is disabled", () => {
  const markup = renderToStaticMarkup(
    createElement(SegmentedControl, {
      defaultValue: "agent",
      options: [
        { label: "Chat", value: "chat" },
        { disabled: true, label: "Agent", value: "agent" },
      ],
    }),
  );

  assert.match(markup, /aria-checked="true"[^>]*data-bf-value="chat"/);
  assert.match(markup, /aria-checked="false"[^>]*data-bf-value="agent"[^>]*disabled=""/);
});

test("SegmentedControl disables every segment as one contract", () => {
  const markup = renderToStaticMarkup(
    createElement(SegmentedControl, { disabled: true, options }),
  );

  assert.match(markup, /aria-disabled="true"/);
  assert.match(markup, /data-disabled="true"/);
  assert.equal((markup.match(/disabled=""/g) ?? []).length, 2);
});

test("SegmentedControl styles bind pill geometry and shared action tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-control-segmented-control-padding/);
  assert.match(styles, /--bf-control-segmented-control-segment-height/);
  assert.match(styles, /--bf-control-segmented-control-segment-padding-inline/);
  assert.match(styles, /--bf-control-segmented-control-icon-size/);
  assert.match(styles, /--bf-type-meta-font-size/);
  assert.match(styles, /--bf-color-action-neutral-surface/);
  assert.match(styles, /--bf-color-surface-raised/);
  assert.match(styles, /--bf-color-action-neutral-surface-hover/);
  assert.match(styles, /--bf-color-action-neutral-content-disabled/);
  assert.match(styles, /--bf-color-focus-ring/);
});
