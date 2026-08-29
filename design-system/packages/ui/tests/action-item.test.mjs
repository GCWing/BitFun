import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ActionItem, KeyHint } from "../dist/index.js";

test("ActionItem exposes native trigger semantics and independent content areas", () => {
  const markup = renderToStaticMarkup(
    createElement(ActionItem, {
      leading: createElement("svg", { "data-icon": "assistant" }),
      reserveLeadingSpace: true,
      role: "menuitem",
      shortcut: createElement(KeyHint, null, "K"),
    }, "AI Assistant"),
  );

  assert.match(markup, /data-bf-component="action-item"/);
  assert.match(markup, /<button[^>]+role="menuitem"/);
  assert.match(markup, /data-bf-part="leading"/);
  assert.match(markup, /data-bf-part="label">AI Assistant<\/span>/);
  assert.match(markup, /data-bf-part="shortcut"/);
  assert.match(markup, /<kbd/);
});

test("ActionItem renders end actions as sibling buttons and disables the whole contract", () => {
  const markup = renderToStaticMarkup(
    createElement(ActionItem, {
      actions: [
        { icon: createElement("svg"), id: "add", label: "Add" },
        { icon: createElement("svg"), id: "more", label: "More" },
      ],
      disabled: true,
    }, "AI Assistant"),
  );

  const firstButtonStart = markup.indexOf("<button");
  const firstButtonEnd = markup.indexOf("</button>", firstButtonStart);
  const secondButtonStart = markup.indexOf("<button", firstButtonStart + 1);

  assert.equal((markup.match(/<button/g) ?? []).length, 3);
  assert.ok(firstButtonEnd < secondButtonStart);
  assert.equal((markup.match(/disabled=""/g) ?? []).length, 3);
  assert.match(markup, /data-bf-part="actions"/);
  assert.match(markup, /aria-label="Add"/);
  assert.match(markup, /aria-label="More"/);
});

test("ActionItem renders trailing metadata between the label and the shortcut", () => {
  const markup = renderToStaticMarkup(
    createElement(ActionItem, {
      metadata: "12",
      shortcut: createElement(KeyHint, null, "K"),
    }, "Sessions"),
  );

  const label = markup.indexOf('data-bf-part="label"');
  const metadata = markup.indexOf('data-bf-part="metadata"');
  const shortcut = markup.indexOf('data-bf-part="shortcut"');

  assert.match(markup, /data-bf-part="metadata"[^>]*>12<\/span>/);
  assert.doesNotMatch(markup, /data-bf-part="metadata"[^>]*aria-hidden/);
  assert.ok(label < metadata);
  assert.ok(metadata < shortcut);
});

test("ActionItem can reserve an empty leading gutter for aligned lists", () => {
  const markup = renderToStaticMarkup(
    createElement(ActionItem, { reserveLeadingSpace: true }, "Aligned item"),
  );

  assert.match(markup, /data-bf-part="leading"><\/span>/);
});

test("ActionItem exposes a danger tone for destructive rows", () => {
  const neutralMarkup = renderToStaticMarkup(
    createElement(ActionItem, null, "Rename"),
  );
  const dangerMarkup = renderToStaticMarkup(
    createElement(ActionItem, { tone: "danger" }, "Delete"),
  );

  assert.match(neutralMarkup, /data-bf-tone="neutral"/);
  assert.match(dangerMarkup, /data-bf-tone="danger"/);
});

test("ActionItem styles share action state and focus tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-color-action-neutral-surface/);
  assert.match(styles, /--bf-color-action-neutral-surface-pressed/);
  assert.match(styles, /--bf-color-action-neutral-content-disabled/);
  assert.match(styles, /--bf-color-focus-ring/);
  assert.match(styles, /--bf-control-height-sm/);
});
