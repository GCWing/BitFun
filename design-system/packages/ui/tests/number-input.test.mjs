import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import test from "node:test";
import { NumberInput } from "../dist/index.js";

const numberInputStyles = readFileSync(
  new URL("../src/components/NumberInput/NumberInput.module.css", import.meta.url),
  "utf8",
);

test("NumberInput exposes a decimal input, unit, and labelled step controls", () => {
  const markup = renderToStaticMarkup(createElement(NumberInput, {
    decrementLabel: "Less",
    incrementLabel: "More",
    onValueChange: () => undefined,
    unit: "%",
    value: 50,
  }));
  assert.match(markup, /inputMode="decimal"/);
  assert.match(markup, /aria-label="Less"/);
  assert.match(markup, /aria-label="More"/);
  assert.match(markup, />%<\/span>/);
  assert.match(markup, /data-openbitfun-component="number-input"/);
});

test("NumberInput exposes canonical size names", () => {
  const markup = renderToStaticMarkup(createElement(NumberInput, {
    onValueChange: () => undefined,
    size: "lg",
    value: 3,
  }));
  assert.match(markup, /data-size="lg"/);
});

test("NumberInput applies its accessible label to the native input", () => {
  const markup = renderToStaticMarkup(createElement(NumberInput, {
    "aria-label": "Font size",
    onValueChange: () => undefined,
    value: 14,
  }));
  assert.match(markup, /aria-label="Font size"/);
});

test("NumberInput forwards Field composition attributes onto its native input", () => {
  const markup = renderToStaticMarkup(createElement(NumberInput, {
    id: "context-window", "aria-describedby": "context-help", "aria-invalid": true,
    required: true, onValueChange: () => undefined, value: 1024,
  }));
  assert.match(markup, /<input[^>]*id="context-window"[^>]*required=""[^>]*aria-describedby="context-help"[^>]*aria-invalid="true"/);
});

test("NumberInput keeps numeric values, units, and step controls in a stable inline layout", () => {
  assert.match(numberInputStyles, /\.input \{[^}]*flex: 1 1 auto;[^}]*font-variant-numeric: tabular-nums;/s);
  assert.match(numberInputStyles, /\.unit \{[^}]*flex: 0 0 auto;[^}]*white-space: nowrap;/s);
  assert.match(numberInputStyles, /\.buttons \{[^}]*flex: 0 0 auto;/s);
  assert.match(numberInputStyles, /\.root\[data-disabled="true"\] \.unit \{ color: var\(--openbitfun-color-content-disabled\); \}/);
});
