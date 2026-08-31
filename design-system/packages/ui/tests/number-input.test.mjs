import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import test from "node:test";
import { NumberInput } from "../dist/index.js";

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
  assert.match(markup, /data-bf-component="number-input"/);
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
