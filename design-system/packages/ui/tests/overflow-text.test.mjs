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
  assert.match(markup, /data-overflow-behavior="fade"/);
  assert.match(markup, />deepseek-v4-pro-with-a-long-suffix<\/span><\/span>/);
});

test("OverflowText exposes the reusable marquee behavior without changing its accessible text", () => {
  const markup = renderToStaticMarkup(createElement(
    OverflowText,
    {
      behavior: "marquee",
      "aria-label": "Complete workspace directory",
    },
    "a-very-long-remote-workspace-directory",
  ));

  assert.match(markup, /data-overflow-behavior="marquee"/);
  assert.match(markup, /--_overflow-text-marquee-distance:0px/);
  assert.match(markup, /aria-label="Complete workspace directory"/);
  assert.match(markup, />a-very-long-remote-workspace-directory<\/span><\/span>/);
});

test("OverflowText measures real clipping for fade and marquee treatments", async () => {
  const [source, styles] = await Promise.all([
    readFile(new URL("../src/primitives/OverflowText/OverflowText.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/primitives/OverflowText/OverflowText.module.css", import.meta.url), "utf8"),
  ]);

  assert.match(source, /content\.scrollWidth - element\.clientWidth/);
  assert.match(source, /measurementRef\.current/);
  assert.match(
    source,
    /useIsomorphicLayoutEffect\(\(\) => \{\s*updateOverflow\(\);\s*\}, \[children, updateOverflow\]\);/s,
  );
  assert.match(source, /new ResizeObserver\(updateOverflow\)/);
  assert.match(source, /resizeObserver\?\.observe\(contentRef\.current\)/);
  assert.match(source, /--_overflow-text-marquee-distance/);
  assert.match(source, /--_overflow-text-marquee-duration/);
  assert.match(styles, /text-overflow:\s*clip/);
  assert.match(styles, /data-overflow-behavior="fade"/);
  assert.match(styles, /data-overflow-behavior="marquee"/);
  assert.match(styles, /--openbitfun-layout-overflow-text-fade-extent/);
  assert.match(styles, /openbitfun-overflow-text-marquee/);
  assert.match(styles, /prefers-reduced-motion:\s*reduce/);
  assert.match(styles, /\.root:dir\(rtl\)/);
  assert.doesNotMatch(styles, /text-overflow:\s*ellipsis/);
});
