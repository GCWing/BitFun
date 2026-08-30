import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { Button } from "../dist/index.js";

test("Button exposes the neutral outline contract by default", () => {
  const markup = renderToStaticMarkup(createElement(Button, null, "Session"));

  assert.match(markup, /data-bf-component="button"/);
  assert.match(markup, /data-bf-tone="neutral"/);
  assert.match(markup, /data-bf-variant="outline"/);
  assert.match(markup, /data-size="md"/);
  assert.match(markup, /type="button"/);
  assert.match(markup, />Session<\/span>/);
});

test("danger tone preserves destructive semantics independently from presentation", async () => {
  const markup = renderToStaticMarkup(
    createElement(Button, { tone: "danger", variant: "fill" }, "Delete"),
  );
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(markup, /data-bf-tone="danger"/);
  assert.match(markup, /data-bf-variant="fill"/);
  assert.match(styles, /\[data-bf-tone=danger\]/);
  assert.match(styles, /--bf-color-status-danger-content/);
  assert.match(styles, /--bf-color-status-danger-surface/);
  assert.match(styles, /--bf-color-status-danger-border/);
});

test("Button renders decorative icon slots and native disabled state", () => {
  const markup = renderToStaticMarkup(
    createElement(
      Button,
      {
        disabled: true,
        leadingIcon: createElement("svg", { "data-icon": "session" }),
        trailingIcon: createElement("svg", { "data-icon": "chevron-down" }),
        variant: "fill",
      },
      "Session",
    ),
  );

  assert.match(markup, /data-bf-variant="fill"/);
  assert.match(markup, /disabled=""/);
  assert.match(markup, /data-icon="session"/);
  assert.match(markup, /data-icon="chevron-down"/);
  assert.equal((markup.match(/aria-hidden="true"/g) ?? []).length >= 3, true);
});

test("loading keeps the accessible label while disabling activation", () => {
  const markup = renderToStaticMarkup(
    createElement(Button, { loading: true }, "Saving"),
  );

  assert.match(markup, /aria-busy="true"/);
  assert.match(markup, /data-loading="true"/);
  assert.match(markup, /disabled=""/);
  assert.match(markup, />Saving<\/span>/);
});

test("fill styles bind to the public variant attribute", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /\[data-bf-variant=fill\]/);
  assert.doesNotMatch(styles, /\[data-variant=fill\]/);
});

test("primary and text variants expose semantic emphasis without changing button anatomy", async () => {
  const primaryMarkup = renderToStaticMarkup(
    createElement(Button, { variant: "primary" }, "Save"),
  );
  const textMarkup = renderToStaticMarkup(
    createElement(Button, { variant: "text" }, "Learn more"),
  );
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(primaryMarkup, /data-bf-variant="primary"/);
  assert.match(textMarkup, /data-bf-variant="text"/);
  assert.match(styles, /--bf-color-action-primary-background/);
  assert.match(styles, /--bf-color-action-primary-content/);
  assert.match(styles, /--bf-color-accent-default/);
  assert.match(styles, /--bf-color-accent-disabled/);
  assert.match(styles, /text-decoration:underline/);
});

test("Button owns the reference pill geometry and typography", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /letter-spacing:var\(--bf-letter-spacing-normal\)/);
  assert.match(styles, /border-radius:var\(--bf-radius-pill\)/);
  assert.match(styles, /--_button-height:\s*var\(--bf-control-height-md\)/);
  assert.match(styles, /--_button-leading-icon-size:\s*16px/);
  assert.match(styles, /--_button-trailing-icon-size:\s*14px/);
  assert.match(styles, /--_button-font-size:\s*var\(--bf-font-size-sm\)/);
  assert.match(styles, /font-family:var\(--bf-font-family-sans\)/);
  assert.match(styles, /font-weight:var\(--bf-font-weight-regular\)/);
  assert.match(styles, /padding-block:var\(--_button-padding-block\)/);
  assert.match(styles, /padding-inline:var\(--_button-padding-inline\)/);
  assert.match(styles, /--_button-padding-inline:\s*var\(--bf-space-5\)/);
  assert.match(
    styles,
    /\[data-size=xs\]\{[^}]*--_button-height:\s*var\(--bf-control-button-xs-height\)/,
  );
  assert.match(
    styles,
    /\[data-size=xs\]\{[^}]*--_button-padding-inline:\s*var\(--bf-control-button-xs-padding-inline\)/,
  );
  assert.match(
    styles,
    /\[data-size=sm\]\{[^}]*--_button-padding-inline:\s*var\(--bf-space-4\)/,
  );
  assert.match(
    styles,
    /\[data-size=lg\]\{[^}]*--_button-padding-inline:\s*var\(--bf-space-6\)/,
  );
  assert.match(
    styles,
    /\[data-size=lg\]\{[^}]*--_button-leading-icon-size:\s*20px/,
  );
  assert.match(styles, /_progress_[^{]+\{[^}]*inline-size:\s*16px;block-size:\s*16px/);
});

test("Button keeps mixed child content vertically centered", async () => {
  const styles = await readFile(
    new URL("../src/components/Button/Button.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /\.label\s*\{[^}]*display:\s*inline-flex/);
  assert.match(styles, /\.label\s*\{[^}]*align-items:\s*center/);
  assert.match(styles, /\.label\s*\{[^}]*gap:\s*var\(--bf-space-1\)/);
  assert.match(styles, /\.label\s*>\s*:where\(svg,\s*img\)\s*\{[^}]*display:\s*block/);
});

test("real and preview hover states share the component rule", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(
    styles,
    /:is\(:hover,\s*\[data-bf-preview-state=(?:"hover"|hover)\]\):not\(:disabled\)/,
  );
});

test("real and preview active states share the semibold component rule", async () => {
  const styles = await readFile(
    new URL("../src/components/Button/Button.module.css", import.meta.url),
    "utf8",
  );

  assert.match(
    styles,
    /:is\(:active,\s*\[data-bf-preview-state=(?:"active"|active)\]\):not\(:disabled\)/,
  );
  assert.match(styles, /font-weight:\s*var\(--bf-font-weight-semibold\)/);
  assert.equal((styles.match(/--bf-font-weight-semibold/g) ?? []).length, 1);
});

test("fill uses neutral semantic state colors and icons inherit content color", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--_button-background:\s*var\(--bf-color-action-neutral-surface\)/);
  assert.match(styles, /--_button-background-hover:\s*var\(--bf-color-action-neutral-surface-hover\)/);
  assert.match(styles, /--_button-background-active:\s*var\(--bf-color-action-neutral-surface-pressed\)/);
  assert.match(styles, /color:currentColor/);
});
