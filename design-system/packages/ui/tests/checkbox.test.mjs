import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import test from "node:test";
import { Checkbox } from "../dist/index.js";

test("Checkbox keeps native semantics and independent content", () => {
  const markup = renderToStaticMarkup(createElement(Checkbox, {
    checked: true,
    description: "Runs at startup",
    label: "Enable hooks",
    readOnly: true,
  }));
  assert.match(markup, /type="checkbox"/);
  assert.match(markup, /checked=""/);
  assert.match(markup, /Enable hooks/);
  assert.match(markup, /Runs at startup/);
  assert.match(markup, /data-openbitfun-component="checkbox"/);
});

test("Checkbox exposes canonical sizes and states", () => {
  const markup = renderToStaticMarkup(createElement(Checkbox, {
    disabled: true,
    invalid: true,
    indeterminate: true,
    size: "sm",
  }));
  assert.match(markup, /data-size="sm"/);
  assert.match(markup, /data-disabled="true"/);
  assert.match(markup, /data-invalid="true"/);
  assert.match(markup, /data-indeterminate="true"/);
});
