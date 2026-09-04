import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { Icon, Select } from "../dist/index.js";

const options = [
  { label: "Ask", testAttributes: { "data-mode": "ask" }, testId: "ask-option", value: "ask" },
  { disabled: true, label: "Plan", value: "plan" },
  { group: "Advanced", label: "Agent", value: 3 },
];

test("Select preserves native selection and grouped option semantics", () => {
  const markup = renderToStaticMarkup(createElement(Select, {
    "aria-label": "Mode",
    options,
    value: "ask",
  }));

  assert.match(markup, /<select/);
  assert.match(markup, /aria-label="Mode"/);
  assert.match(markup, /<option[^>]*value="ask"[^>]*selected="">Ask<\/option>/);
  assert.match(markup, /data-testid="ask-option"/);
  assert.match(markup, /data-mode="ask"/);
  assert.match(markup, /<option disabled="" value="plan">Plan<\/option>/);
  assert.match(markup, /<optgroup label="Advanced">/);
  assert.match(markup, /value="3">Agent<\/option>/);
});

test("Select exposes size, invalid, disabled, and leading regions independently", () => {
  const markup = renderToStaticMarkup(createElement(Select, {
    "aria-label": "Mode",
    disabled: true,
    invalid: true,
    leading: createElement(Icon, { name: "circle" }),
    options,
    size: "lg",
  }));

  assert.match(markup, /data-size="lg"/);
  assert.match(markup, /data-disabled="true"/);
  assert.match(markup, /data-invalid="true"/);
  assert.match(markup, /aria-invalid="true"/);
  assert.match(markup, /data-openbitfun-part="leading"/);
  assert.match(markup, /data-openbitfun-part="indicator"/);
});

test("Select styles theme both the closed field and native option menu", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--openbitfun-control-select-padding-inline/);
  assert.match(styles, /--openbitfun-control-select-indicator-size/);
  assert.match(styles, /--openbitfun-color-field-border-focus/);
  assert.match(styles, /option[^}]*color:\s*var\(--openbitfun-color-content-primary\)/s);
  assert.match(styles, /option[^}]*background-color:\s*var\(--openbitfun-color-surface-panel\)/s);
  assert.match(styles, /option:disabled[^}]*color:\s*var\(--openbitfun-color-content-disabled\)/s);
  assert.match(styles, /--openbitfun-color-status-danger-border/);
});
