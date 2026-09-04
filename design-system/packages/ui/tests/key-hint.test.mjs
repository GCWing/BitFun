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
  assert.match(markup, /data-openbitfun-component="key-hint"/);
  assert.match(markup, /data-openbitfun-part="icon"/);
  assert.match(markup, /aria-hidden="true"/);
  assert.match(markup, /data-icon="command"/);
  assert.match(markup, /data-openbitfun-part="label">K<\/span>/);
});

test("KeyHint preserves textual platform modifiers in the icon slot", () => {
  const markup = renderToStaticMarkup(
    createElement(KeyHint, { icon: "Ctrl" }, "K"),
  );

  assert.match(markup, /data-openbitfun-part="icon">Ctrl<\/span>/);
  assert.match(markup, /data-openbitfun-part="label">K<\/span>/);
});

test("KeyHint styles use the shared micro typography role", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--openbitfun-color-key-hint-background/);
  assert.match(styles, /--openbitfun-color-action-neutral-surface/);
  assert.match(styles, /--openbitfun-color-content-muted/);
  assert.match(styles, /--openbitfun-type-micro-font-family/);
  assert.match(styles, /--openbitfun-type-micro-font-size/);
  assert.match(styles, /--openbitfun-type-micro-letter-spacing/);
  assert.match(styles, /--openbitfun-type-micro-line-height/);
  assert.match(styles, /--openbitfun-radius-xs/);
  assert.match(styles, /--openbitfun-radius-sm/);
  assert.match(styles, /flex:\s*0 0 auto/);
});
