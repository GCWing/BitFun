import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { OverflowText } from "../dist/index.js";

test("OverflowText preserves the complete accessible text while exposing overflow state", () => {
  const markup = renderToStaticMarkup(createElement(
    OverflowText,
    {
      "aria-label": "Complete model name",
      className: "model-name",
    },
    "deepseek-v4-pro-with-a-long-suffix",
  ));

  assert.match(markup, /class="[^" ]+ model-name"/);
  assert.match(markup, /aria-label="Complete model name"/);
  assert.match(markup, /data-overflow="false"/);
  assert.match(markup, />deepseek-v4-pro-with-a-long-suffix<\/span>/);
});

test("OverflowText applies an inline-end mask only after real clipping is measured", async () => {
  const [source, styles] = await Promise.all([
    readFile(new URL("../src/primitives/OverflowText/OverflowText.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/primitives/OverflowText/OverflowText.module.css", import.meta.url), "utf8"),
  ]);

  assert.match(source, /scrollWidth > element\.clientWidth/);
  assert.match(source, /new ResizeObserver\(updateOverflow\)/);
  assert.match(styles, /text-overflow:\s*clip/);
  assert.match(styles, /\.root\[data-overflow="true"\]/);
  assert.match(styles, /--bf-layout-overflow-text-fade-extent/);
  assert.match(styles, /\.root:dir\(rtl\)\[data-overflow="true"\]/);
  assert.doesNotMatch(styles, /text-overflow:\s*ellipsis/);
});
