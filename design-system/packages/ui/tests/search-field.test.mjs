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

  assert.match(markup, /data-openbitfun-component="search-field"/);
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

test("SearchField renders custom trailing content before the clear action", () => {
  const markup = renderToStaticMarkup(
    createElement(SearchField, {
      "aria-label": "Search",
      clearLabel: "Clear search",
      onClear: () => {},
      trailing: createElement("span", { "data-part": "matches" }, "1 / 5"),
      value: "query",
    }),
  );

  const trailingIndex = markup.indexOf('data-part="matches"');
  const clearIndex = markup.indexOf('aria-label="Clear search"');
  assert.ok(trailingIndex >= 0);
  assert.ok(clearIndex >= 0);
  assert.ok(trailingIndex < clearIndex);
});

test("SearchField exposes a labeled clear action without hiding it from assistive technology", () => {
  const markup = renderToStaticMarkup(
    createElement(SearchField, {
      "aria-label": "Search",
      clearLabel: "Clear search",
      onClear: () => {},
      value: "query",
    }),
  );

  assert.match(markup, /aria-label="Clear search"/);
  assert.match(markup, /data-openbitfun-component="icon-button"/);
  assert.match(markup, /data-openbitfun-shape="circle"/);
  assert.match(markup, /data-size="xs"/);
});

test("SearchField keeps its clear action inset, background-free, and focus-preserving", async () => {
  const source = await readFile(
    new URL("../src/components/SearchField/SearchField.tsx", import.meta.url),
    "utf8",
  );
  const styles = await readFile(
    new URL("../src/components/SearchField/SearchField.module.css", import.meta.url),
    "utf8",
  );

  assert.match(source, /onMouseDown=\{\(event\) => event\.preventDefault\(\)\}/);
  assert.match(
    styles,
    /\.root \.clear\s*\{[^}]*--_icon-button-background:\s*transparent[^}]*--_icon-button-background-hover:\s*transparent[^}]*--_icon-button-background-active:\s*transparent[^}]*background:\s*transparent/s,
  );
});

test("SearchField owns pill composition while reusing Input behavior", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /border-radius:var\(--openbitfun-radius-pill\)/);
  assert.match(styles, /--openbitfun-type-label-md-font-size/);
  assert.match(styles, /--openbitfun-type-meta-font-size/);
});

test("SearchField focus changes only the existing border color", async () => {
  const styles = await readFile(
    new URL("../src/components/SearchField/SearchField.module.css", import.meta.url),
    "utf8",
  );
  const focusRule = styles.match(
    /\.root \.field:not\(\[data-invalid="true"\]\):focus-within\s*\{([^}]+)\}/,
  )?.[1];

  assert.ok(focusRule);
  assert.match(focusRule, /border-color: var\(--openbitfun-color-content-primary\)/);
  assert.match(focusRule, /box-shadow: none/);
  assert.doesNotMatch(focusRule, /border-width|outline/);
});
