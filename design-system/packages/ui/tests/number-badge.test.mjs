import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { NumberBadge, ToolbarBadge } from "../dist/index.js";

test("NumberBadge renders caller-owned values, including zero and long counts", () => {
  for (const value of [0, 18, 1234, "99+"]) {
    const markup = renderToStaticMarkup(createElement(NumberBadge, { value, "aria-label": `${value} items` }));
    assert.match(markup, /data-openbitfun-component="number-badge"/);
    assert.ok(markup.includes(`>${value}</span>`));
    assert.ok(markup.includes(`aria-label="${value} items"`));
    assert.doesNotMatch(markup, /aria-hidden/);
  }
});

test("ToolbarBadge preserves its slot hook and delegates to NumberBadge", () => {
  const markup = renderToStaticMarkup(createElement(ToolbarBadge, { className: "product-count", title: "Items" }, 18));
  assert.match(markup, /data-openbitfun-part="badge"/);
  assert.match(markup, /data-openbitfun-component="number-badge"/);
  assert.match(markup, /product-count/);
  assert.match(markup, /data-openbitfun-part="value">18<\/span>/);
});

test("NumberBadge uses a 24px slot, 20px surface and 11px control typography without clipping", async () => {
  const source = await readFile(new URL("../src/components/NumberBadge/NumberBadge.module.css", import.meta.url), "utf8");
  for (const token of ["space-6", "space-5", "type-meta-font-size", "type-meta-font-family", "type-meta-font-weight", "color-action-neutral-content", "color-action-neutral-surface"]) assert.ok(source.includes(`--openbitfun-${token}`), token);
  assert.doesNotMatch(source, /overflow:\s*(hidden|clip)|text-overflow:\s*ellipsis/);
  assert.match(source, /forced-colors: active/);
});
