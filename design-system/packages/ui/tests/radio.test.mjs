import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import test from "node:test";
import { Radio } from "../dist/index.js";

test("Radio keeps native semantics and supporting copy", () => {
  const markup = renderToStaticMarkup(createElement(Radio, {
    checked: true,
    description: "Uses the recommended model",
    label: "Primary",
    name: "model",
    readOnly: true,
    value: "primary",
  }));
  assert.match(markup, /type="radio"/);
  assert.match(markup, /checked=""/);
  assert.match(markup, /name="model"/);
  assert.match(markup, /Primary/);
  assert.match(markup, /Uses the recommended model/);
  assert.match(markup, /data-bf-component="radio"/);
});

test("Radio exposes normalized sizes and states", () => {
  const markup = renderToStaticMarkup(createElement(Radio, {
    disabled: true,
    invalid: true,
    label: "Unavailable",
    size: "sm",
  }));
  assert.match(markup, /data-size="sm"/);
  assert.match(markup, /data-disabled="true"/);
  assert.match(markup, /data-invalid="true"/);
});
