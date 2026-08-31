import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { Textarea } from "../dist/index.js";

test("Textarea keeps native semantics, labels, support text, and count", () => {
  const markup = renderToStaticMarkup(createElement(Textarea, {
    label: "Instructions",
    maxLength: 80,
    showCount: true,
    value: "Review this change",
    readOnly: true,
  }));
  assert.match(markup, /<textarea/);
  assert.match(markup, /Instructions/);
  assert.match(markup, /18 \/ 80/);
  assert.match(markup, /data-bf-component="textarea"/);
});

test("Textarea guards IME-owned commit and cancel keys", async () => {
  const source = await readFile(new URL("../src/components/Textarea/Textarea.tsx", import.meta.url), "utf8");
  assert.match(source, /isImeOwnedKeyboardEvent/);
  assert.match(source, /event\.stopPropagation\(\)/);
  assert.match(source, /onCompositionStart/);
  assert.match(source, /onCompositionEnd/);
});

test("Textarea resizes on mount and when a controlled value changes", async () => {
  const source = await readFile(new URL("../src/components/Textarea/Textarea.tsx", import.meta.url), "utf8");
  assert.match(source, /useIsomorphicLayoutEffect/);
  assert.match(source, /\[resizeToContent, value\]/);
  assert.match(source, /resizeToContent\(textareaRef\.current\)/);
});
