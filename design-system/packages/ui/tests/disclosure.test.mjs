import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Disclosure } from "../dist/index.js";

test("Disclosure connects its trigger and region with accessible state", () => {
  const markup = renderToStaticMarkup(createElement(
    Disclosure,
    { defaultOpen: true, summary: "Advanced" },
    createElement("button", null, "Reset"),
  ));

  assert.match(markup, /data-bf-component="disclosure"/);
  assert.match(markup, /data-open="true"/);
  assert.match(markup, /aria-expanded="true"/);
  assert.match(markup, /aria-controls="bf-disclosure-[^"]+-content"/);
  assert.match(markup, /role="region"/);
  assert.match(markup, /aria-labelledby="bf-disclosure-[^"]+-trigger"/);
  assert.doesNotMatch(markup, /inert=""/);
});

test("Disclosure removes collapsed content from sequential focus navigation", () => {
  const markup = renderToStaticMarkup(createElement(
    Disclosure,
    { summary: "Advanced" },
    createElement("button", null, "Reset"),
  ));

  assert.match(markup, /data-open="false"/);
  assert.match(markup, /aria-expanded="false"/);
  assert.match(markup, /aria-hidden="true"/);
  assert.match(markup, /inert=""/);
});

test("Disclosure uses public geometry tokens and honors reduced motion", async () => {
  const styles = await readFile(
    new URL("../src/components/Disclosure/Disclosure.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /--bf-layout-disclosure-trigger-min-block-size/);
  assert.match(styles, /--bf-layout-disclosure-content-padding-inline/);
  assert.match(styles, /prefers-reduced-motion: reduce/);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});
