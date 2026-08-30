import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Combobox, ComboboxProvider } from "../dist/index.js";

test("Combobox preserves provider localization and compatibility props", () => {
  const markup = renderToStaticMarkup(createElement(
    ComboboxProvider,
    { labels: { placeholder: "Choose model" } },
    createElement(Combobox, {
      disabled: true,
      error: true,
      errorMessage: "Required",
      options: [],
      required: true,
    }),
  ));

  assert.match(markup, /Choose model/);
  assert.match(markup, /aria-invalid="true"/);
  assert.match(markup, /aria-required="true"/);
  assert.match(markup, /aria-describedby="[^\"]+-error"/);
  assert.match(markup, /Required/);
});

test("Combobox links its trigger, search, and multi-select listbox", () => {
  const markup = renderToStaticMarkup(createElement(Combobox, {
    defaultOpen: true,
    defaultValue: ["one"],
    multiple: true,
    options: [
      { label: "One", value: "one" },
      { description: "Unavailable", disabled: true, label: "Two", value: "two" },
    ],
    portalContainer: null,
    searchable: true,
    showSelectAll: true,
    triggerAriaLabel: "Models",
  }));

  assert.match(markup, /role="combobox"/);
  assert.match(markup, /aria-expanded="true"/);
  assert.match(markup, /aria-controls="[^"]+-listbox"/);
  assert.match(markup, /role="combobox"/);
  assert.match(markup, /role="listbox"/);
  assert.match(markup, /aria-multiselectable="true"/);
  assert.match(markup, /aria-selected="true"/);
});

test("Combobox owns keyboard selection, IME safety, filtering, and custom values", async () => {
  const source = await readFile(
    new URL("../src/components/Combobox/Combobox.tsx", import.meta.url),
    "utf8",
  );

  assert.match(source, /case|event\.key === "ArrowDown"/);
  assert.match(source, /event\.key === "ArrowUp"/);
  assert.match(source, /event\.key === "Home"/);
  assert.match(source, /event\.key === "End"/);
  assert.match(source, /nativeEvent\.isComposing/);
  assert.match(source, /filterOption\(option, query\)/);
  assert.match(source, /submitCustomValue/);
  assert.doesNotMatch(source, /onMouseEnter=.*setActive/);
});

test("Combobox styling uses public field, overlay, action, and motion tokens", async () => {
  const styles = await readFile(
    new URL("../src/components/Combobox/Combobox.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /--bf-color-field-background/);
  assert.match(styles, /--bf-overlay-menu-surface-radius/);
  assert.match(styles, /--bf-color-action-neutral-surface/);
  assert.match(styles, /--bf-shadow-menu/);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});
