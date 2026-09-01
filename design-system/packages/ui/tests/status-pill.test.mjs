import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { Icon, StatusPill } from "../dist/index.js";

test("StatusPill exposes semantic tone and independent label anatomy", () => {
  const markup = renderToStaticMarkup(createElement(StatusPill, { tone: "warning" }, "Review"));

  assert.match(markup, /data-bf-component="status-pill"/);
  assert.match(markup, /data-tone="warning"/);
  assert.match(markup, /data-bf-part="label"/);
  assert.match(markup, />Review</);
});

test("StatusPill renders the brand accent tone", () => {
  const markup = renderToStaticMarkup(createElement(StatusPill, { tone: "accent" }, "Builtin"));

  assert.match(markup, /data-tone="accent"/);
});

test("StatusPill keeps its optional leading indicator decorative", () => {
  const markup = renderToStaticMarkup(createElement(
    StatusPill,
    { leading: createElement(Icon, { name: "circle" }) },
    "Ask",
  ));

  assert.match(markup, /data-tone="success"/);
  assert.match(markup, /data-bf-part="leading"/);
  assert.match(markup, /aria-hidden="true"/);
});

test("StatusPill styles consume public semantic and geometry tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-control-status-pill-gap/);
  assert.match(styles, /--bf-control-status-pill-padding-block/);
  assert.match(styles, /--bf-control-status-pill-icon-size/);
  assert.match(styles, /--bf-color-status-success-surface/);
  assert.match(styles, /--bf-color-status-danger-content/);
  assert.match(styles, /text-overflow:\s*ellipsis/);
});
