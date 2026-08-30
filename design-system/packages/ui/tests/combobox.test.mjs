import assert from "node:assert/strict";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Combobox, ComboboxProvider } from "../dist/index.js";

test("Combobox preserves typed selections and accessible labels", () => {
  const html = renderToStaticMarkup(createElement(Combobox, { label: "Models", multiple: true, value: [0, "custom"], options: [{ label: "Zero", value: 0 }] }));
  assert.match(html, /role="combobox"/);
  assert.match(html, /aria-expanded="false"/);
  assert.match(html, /Zero, custom/);
  assert.match(html, /data-multiple="true"/);
});
test("Combobox localizes defaults through its provider and exposes invalid state", () => {
  const html = renderToStaticMarkup(createElement(ComboboxProvider, { labels: { placeholder: "Choose model" } }, createElement(Combobox, { disabled: true, error: true, errorMessage: "Required" })));
  assert.match(html, /Choose model/);
  assert.match(html, /aria-invalid="true"/);
  assert.match(html, /aria-describedby="bf-combobox-[^"]+-error"/);
  assert.match(html, /disabled=""/);
});
