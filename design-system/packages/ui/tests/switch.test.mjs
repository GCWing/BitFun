import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Switch } from "../dist/index.js";

test("Switch exposes native checkbox and switch semantics", () => {
  const markup = renderToStaticMarkup(
    createElement(Switch, { "aria-label": "Notifications" }),
  );

  assert.match(markup, /data-openbitfun-component="switch"/);
  assert.match(markup, /aria-label="Notifications"/);
  assert.match(markup, /role="switch"/);
  assert.match(markup, /type="checkbox"/);
});

test("Switch supports an initially checked uncontrolled state", () => {
  const markup = renderToStaticMarkup(
    createElement(Switch, {
      "aria-label": "Notifications",
      defaultChecked: true,
    }),
  );

  assert.match(markup, /checked=""/);
});

test("Switch consumes the reference geometry tokens", async () => {
  const styles = await readFile(
    new URL("../src/components/Switch/Switch.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /inline-size: var\(--openbitfun-control-switch-track-width\);/);
  assert.match(styles, /block-size: var\(--openbitfun-control-switch-track-height\);/);
  assert.match(styles, /inline-size: var\(--openbitfun-control-switch-thumb-size\);/);
  assert.match(styles, /block-size: var\(--openbitfun-control-switch-thumb-size\);/);
  assert.match(styles, /inset-inline-start: var\(--openbitfun-control-switch-thumb-inset\);/);
  assert.match(styles, /translateX\(var\(--openbitfun-control-switch-thumb-travel\)\)/);
  assert.match(styles, /:dir\(rtl\)/);
  assert.match(styles, /translateX\(var\(--openbitfun-control-switch-thumb-travel-reverse\)\)/);
  assert.doesNotMatch(styles, /\b(?:1|2|12|16|28)px\b/);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});
