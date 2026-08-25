import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import test from "node:test";
import { IconButton } from "../dist/index.js";

test("IconButton exposes the Figma neutral small contract by default", () => {
  const markup = renderToStaticMarkup(
    createElement(
      IconButton,
      { "aria-label": "Show list" },
      createElement("svg", { "data-icon": "list" }),
    ),
  );

  assert.match(markup, /aria-label="Show list"/);
  assert.match(markup, /data-bf-component="icon-button"/);
  assert.match(markup, /data-bf-tone="neutral"/);
  assert.match(markup, /data-size="sm"/);
  assert.match(markup, /type="button"/);
  assert.match(markup, /data-icon="list"/);
  assert.equal((markup.match(/aria-hidden="true"/g) ?? []).length, 2);
});

test("loading preserves the accessible name and disables activation", () => {
  const markup = renderToStaticMarkup(
    createElement(
      IconButton,
      { "aria-label": "Refresh", loading: true },
      createElement("svg"),
    ),
  );

  assert.match(markup, /aria-busy="true"/);
  assert.match(markup, /data-loading="true"/);
  assert.match(markup, /disabled=""/);
  assert.match(markup, /aria-label="Refresh"/);
});

test("IconButton consumes semantic state colors and component geometry", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-control-icon-button-size-sm/);
  assert.match(styles, /--bf-control-icon-button-icon-size-sm/);
  assert.match(styles, /--bf-control-icon-button-radius/);
  assert.match(styles, /--bf-color-action-neutral-surface/);
  assert.match(styles, /--bf-color-action-neutral-surface-pressed/);
  assert.match(styles, /--bf-color-status-danger-content/);
  assert.match(styles, /--bf-color-accent-default/);
});

test("real and preview interaction states share component selectors", async () => {
  const styles = await readFile(
    new URL("../src/components/IconButton/IconButton.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /:is\(:hover,\s*\[data-bf-preview-state="hover"\]\):not\(:disabled\)/);
  assert.match(styles, /:is\(:active,\s*\[data-bf-preview-state="active"\]\):not\(:disabled\)/);
  assert.match(styles, /@media \(prefers-reduced-motion: reduce\)/);
  assert.match(styles, /@media \(forced-colors: active\)/);
});
