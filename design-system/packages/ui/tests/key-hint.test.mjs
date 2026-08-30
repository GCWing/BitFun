import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { KeyHint } from "../dist/index.js";

test("KeyHint preserves keyboard semantics and optional icon anatomy", () => {
  const markup = renderToStaticMarkup(
    createElement(KeyHint, {
      icon: createElement("svg", { "data-icon": "command" }),
    }, "K"),
  );

  assert.match(markup, /^<kbd/);
  assert.match(markup, /data-bf-component="key-hint"/);
  assert.match(markup, /data-bf-part="icon"/);
  assert.match(markup, /aria-hidden="true"/);
  assert.match(markup, /data-icon="command"/);
  assert.match(markup, /data-bf-part="label">K<\/span>/);
});

test("KeyHint preserves textual platform modifiers in the icon slot", () => {
  const markup = renderToStaticMarkup(
    createElement(KeyHint, { icon: "Ctrl" }, "K"),
  );

  assert.match(markup, /data-bf-part="icon">Ctrl<\/span>/);
  assert.match(markup, /data-bf-part="label">K<\/span>/);
});

test("KeyHint styles use shared primitive tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-color-key-hint-background/);
  assert.match(styles, /--bf-color-action-neutral-surface/);
  assert.match(styles, /--bf-color-content-muted/);
  assert.match(styles, /--bf-font-family-sans/);
  assert.match(styles, /--bf-font-size-micro/);
  assert.match(styles, /--bf-letter-spacing-normal/);
  assert.match(styles, /--bf-line-height-none/);
  assert.match(styles, /--bf-radius-xs/);
  assert.match(styles, /--bf-radius-sm/);
  assert.match(styles, /flex:\s*0 0 auto/);
});
