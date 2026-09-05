import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Combobox, DesignSystemProvider, MultiSelect } from "../dist/index.js";

test("Combobox consumes host localization and canonical invalid state", () => {
  const markup = renderToStaticMarkup(createElement(
    DesignSystemProvider,
    { messages: { selectPlaceholder: "Choose model" } },
    createElement(Combobox, {
      disabled: true,
      invalid: true,
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

test("MultiSelect exposes an explicit multi-value trigger contract", () => {
  const markup = renderToStaticMarkup(createElement(MultiSelect, {
    defaultOpen: true,
    defaultValue: ["one"],
    options: [
      { label: "One", value: "one" },
      { description: "Unavailable", disabled: true, label: "Two", value: "two" },
    ],
    showSelectAll: true,
    "aria-label": "Models",
  }));

  assert.match(markup, /data-openbitfun-component="multi-select"/);
  assert.match(markup, /role="combobox"/);
  assert.match(markup, /aria-expanded="true"/);
  assert.match(markup, /aria-label="Models"/);
  assert.match(markup, />One</);
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
  assert.match(source, /submitCreateValue/);
  assert.match(source, /onCreateValue/);
  assert.match(source, /useDismissibleLayer/);
  assert.doesNotMatch(source, /onMouseEnter=.*setActive/);
});

test("Combobox styling uses public field, overlay, action, and motion tokens", async () => {
  const source = await readFile(
    new URL("../src/components/Combobox/Combobox.tsx", import.meta.url),
    "utf8",
  );
  const styles = await readFile(
    new URL("../src/components/Combobox/Combobox.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /--openbitfun-color-field-background/);
  assert.match(styles, /--openbitfun-overlay-menu-surface-radius/);
  assert.match(styles, /--openbitfun-color-surface-tertiary/);
  assert.match(styles, /--openbitfun-shadow-menu/);
  assert.match(styles, /position:\s*fixed/);
  assert.match(styles, /z-index:\s*var\(--openbitfun-layer-popover\)/);
  assert.match(source, /className=\{styles\.searchField\}/);
  assert.match(styles, /\.searchField\s*\{[^}]*inline-size:\s*100%/);
  assert.doesNotMatch(styles, /data-popover-mode/);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});

test("Combobox search and option geometry follows the standard menu rhythm", async () => {
  const source = await readFile(
    new URL("../src/components/Combobox/Combobox.tsx", import.meta.url),
    "utf8",
  );
  const styles = await readFile(
    new URL("../src/components/Combobox/Combobox.module.css", import.meta.url),
    "utf8",
  );

  assert.match(source, /clearLabel=\{query \? designSystem\.messages\.clearSelection : undefined\}/);
  assert.match(source, /leadingIcon=\{<Icon name="search" \/>\}/);
  assert.match(styles, /\.popover\s*\{[^}]*gap:\s*var\(--openbitfun-space-1\)/s);
  assert.match(
    styles,
    /\.searchField \[data-openbitfun-component="input"\]\s*\{[^}]*border-radius:\s*var\(--openbitfun-control-select-radius\)/s,
  );
  assert.match(
    styles,
    /\.listbox \[data-openbitfun-part="list"\],[^}]*gap:\s*calc\(var\(--openbitfun-space-1\) \/ 2\)/s,
  );
});
