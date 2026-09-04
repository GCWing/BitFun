import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ActivityItem, ChangeCount } from "../dist/index.js";

test("ActivityItem keeps identity, content, metadata, and actions independent", () => {
  const markup = renderToStaticMarkup(
    createElement(ActivityItem, {
      actions: [
        { icon: createElement("svg"), id: "copy", label: "Copy" },
        { icon: createElement("svg"), id: "download", label: "Download" },
      ],
      appearance: "surface",
      label: "Run command",
      leading: createElement("svg", { "data-icon": "terminal" }),
      metadata: createElement(ChangeCount, { additions: 6, deletions: 2 }),
    }, "pnpm run check"),
  );

  assert.match(markup, /data-openbitfun-component="activity-item"/);
  assert.match(markup, /data-appearance="surface"/);
  assert.match(markup, /data-openbitfun-part="leading"/);
  assert.match(markup, /data-openbitfun-part="label">Run command<\/span>/);
  assert.match(markup, /data-openbitfun-part="description">pnpm run check<\/span>/);
  assert.match(markup, /data-openbitfun-part="metadata"/);
  assert.match(markup, /data-openbitfun-part="divider"/);
  assert.match(markup, /data-openbitfun-part="actions"/);
  assert.equal((markup.match(/<button/g) ?? []).length, 2);
});

test("ActivityItem exposes an optional native trigger without nesting sibling actions", () => {
  const markup = renderToStaticMarkup(
    createElement(ActivityItem, {
      actions: [{ icon: createElement("svg"), id: "open", label: "Open" }],
      onActivate: () => undefined,
    }, "Read file"),
  );

  const triggerEnd = markup.indexOf("</button>");
  const siblingAction = markup.indexOf("<button", markup.indexOf("<button") + 1);

  assert.equal((markup.match(/<button/g) ?? []).length, 2);
  assert.match(markup, /<button[^>]+data-openbitfun-part="trigger"/);
  assert.ok(triggerEnd < siblingAction);
});

test("ActivityItem disables its trigger and sibling actions as one contract", () => {
  const markup = renderToStaticMarkup(
    createElement(ActivityItem, {
      actions: [{ icon: createElement("svg"), id: "copy", label: "Copy" }],
      disabled: true,
      onActivate: () => undefined,
    }, "Unavailable activity"),
  );

  assert.match(markup, /aria-disabled="true"/);
  assert.match(markup, /data-disabled="true"/);
  assert.equal((markup.match(/disabled=""/g) ?? []).length, 2);
});

test("ActivityItem renders an optional full-width detail area after the row content", () => {
  const markup = renderToStaticMarkup(
    createElement(ActivityItem, {
      actions: [{ icon: createElement("svg"), id: "open", label: "Open" }],
      appearance: "surface",
      detail: createElement("pre", null, "+ registry.register(component)"),
      label: "Edit file",
    }, "src/registry.ts"),
  );

  const description = markup.indexOf('data-openbitfun-part="description"');
  const actions = markup.indexOf('data-openbitfun-part="actions"');
  const detail = markup.indexOf('data-openbitfun-part="detail"');

  assert.match(markup, /data-has-detail="true"/);
  assert.match(markup, /data-openbitfun-part="detail"[^>]*><pre>\+ registry\.register\(component\)<\/pre>/);
  assert.ok(description < actions);
  assert.ok(actions < detail);
});

test("ActivityItem reports the absence of a detail area on the root contract", () => {
  const markup = renderToStaticMarkup(
    createElement(ActivityItem, { appearance: "surface" }, "pnpm run check"),
  );

  assert.match(markup, /data-has-detail="false"/);
  assert.doesNotMatch(markup, /data-openbitfun-part="detail"/);
});

test("ChangeCount formats positive additions and deletions with distinct parts", () => {
  const markup = renderToStaticMarkup(
    createElement(ChangeCount, { additions: -6, deletions: -2 }),
  );

  assert.match(markup, /data-openbitfun-component="change-count"/);
  assert.match(markup, /data-openbitfun-part="additions">\+6<\/span>/);
  assert.match(markup, /data-openbitfun-part="deletions">-2<\/span>/);
});

test("ActivityItem styles use public activity, code-change, and focus tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--openbitfun-control-activity-item-surface-height/);
  assert.match(styles, /--openbitfun-control-activity-item-inline-icon-size/);
  assert.match(styles, /--openbitfun-control-change-count-padding-block/);
  assert.match(styles, /--openbitfun-control-icon-button-xs-size/);
  assert.match(styles, /--openbitfun-color-code-change-added/);
  assert.match(styles, /--openbitfun-color-code-change-removed/);
  assert.match(styles, /--openbitfun-color-focus-ring/);
});
