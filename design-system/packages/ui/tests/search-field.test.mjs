import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { SearchField } from "../dist/index.js";

test("SearchField composes search semantics with icon and shortcut slots", () => {
  const markup = renderToStaticMarkup(
    createElement(SearchField, {
      "aria-label": "Search",
      leadingIcon: createElement("svg", { "data-icon": "search" }),
      placeholder: "Search",
      shortcut: "Ctrl K",
    }),
  );

  assert.match(markup, /data-bf-component="search-field"/);
  assert.match(markup, /type="search"/);
  assert.match(markup, /data-icon="search"/);
  assert.match(markup, /Ctrl K/);
  assert.equal((markup.match(/aria-hidden="true"/g) ?? []).length, 2);
});

test("SearchField source preserves consumer key handling before Enter submission", async () => {
  const source = await readFile(
    new URL("../src/components/SearchField/SearchField.tsx", import.meta.url),
    "utf8",
  );

  assert.match(source, /onKeyDown\?\.\(event\)/);
  assert.match(source, /!event\.defaultPrevented && event\.key === "Enter"/);
  assert.match(source, /onSearch\?\.\(event\.currentTarget\.value\)/);
});

test("SearchField owns pill composition while reusing Input behavior", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /border-radius:var\(--bf-radius-pill\)/);
  assert.match(styles, /--bf-font-size-caption/);
});
