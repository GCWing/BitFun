import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import test from "node:test";
import { NumberInput } from "../dist/index.js";

test("NumberInput exposes a decimal input, unit, and labelled step controls", () => {
  const markup = renderToStaticMarkup(createElement(NumberInput, {
    decrementLabel: "Less",
    incrementLabel: "More",
    onChange: () => undefined,
    unit: "%",
    value: 50,
  }));
  assert.match(markup, /inputMode="decimal"/);
  assert.match(markup, /aria-label="Less"/);
  assert.match(markup, /aria-label="More"/);
  assert.match(markup, />%<\/span>/);
  assert.match(markup, /data-bf-component="number-input"/);
});

test("NumberInput normalizes legacy size names", () => {
  const markup = renderToStaticMarkup(createElement(NumberInput, {
    onChange: () => undefined,
    size: "large",
    value: 3,
  }));
  assert.match(markup, /data-size="lg"/);
});

test("NumberInput forwards native input attributes", () => {
  const markup = renderToStaticMarkup(createElement(NumberInput, {
    inputProps: { "aria-label": "Font size", "data-testid": "font-size" },
    onChange: () => undefined,
    value: 14,
  }));
  assert.match(markup, /aria-label="Font size"/);
  assert.match(markup, /data-testid="font-size"/);
});

test("NumberInput forwards Field composition attributes onto its native input", () => {
  const markup = renderToStaticMarkup(createElement(NumberInput, {
    id: "context-window", "aria-describedby": "context-help", "aria-invalid": true,
    required: true, onChange: () => undefined, value: 1024,
  }));
  assert.match(markup, /<input[^>]*id="context-window"[^>]*required=""[^>]*aria-describedby="context-help"[^>]*aria-invalid="true"/);
});
