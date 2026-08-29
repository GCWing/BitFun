import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ScrollArea } from "../dist/index.js";

test("ScrollArea defaults to a native vertical viewport with automatic visibility", () => {
  const markup = renderToStaticMarkup(
    createElement(ScrollArea, { "aria-label": "Activity" }, "Content"),
  );

  assert.match(markup, /data-bf-component="scroll-area"/);
  assert.match(markup, /data-bf-part="viewport"/);
  assert.match(markup, /data-bf-orientation="vertical"/);
  assert.match(markup, /data-bf-scrollbar-visibility="auto"/);
  assert.match(markup, /aria-label="Activity"/);
});

test("ScrollArea exposes orientation and scrollbar visibility contracts", () => {
  const markup = renderToStaticMarkup(
    createElement(
      ScrollArea,
      { orientation: "both", scrollbarVisibility: "always" },
      "Content",
    ),
  );

  assert.match(markup, /data-bf-orientation="both"/);
  assert.match(markup, /data-bf-scrollbar-visibility="always"/);
});

test("ScrollArea preserves feature-owned appearance contracts", () => {
  const markup = renderToStaticMarkup(
    createElement(
      ScrollArea,
      { "data-bf-component": "model-settings", "data-bf-part": "root" },
      "Content",
    ),
  );

  assert.match(markup, /data-bf-component="model-settings"/);
  assert.match(markup, /data-bf-part="root"/);
});

test("ScrollArea styling uses public scrollbar tokens and preserves native scrolling", async () => {
  const styles = await readFile(
    new URL("../src/components/ScrollArea/ScrollArea.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /overflow-y: auto/);
  assert.match(styles, /overflow-x: auto/);
  assert.match(styles, /overflow: scroll/);
  assert.match(styles, /--bf-scrollbar-width/);
  assert.match(styles, /--bf-scrollbar-radius/);
  assert.match(styles, /--bf-color-scrollbar-thumb/);
  assert.match(styles, /--bf-color-scrollbar-thumb-hover/);
  assert.match(styles, /scrollbar-width: none/);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});
