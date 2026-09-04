import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Icon, LauncherButton } from "../dist/index.js";

test("LauncherButton exposes native button semantics and stable icon/label parts", () => {
  const markup = renderToStaticMarkup(
    createElement(
      LauncherButton,
      {
        "aria-label": "Start a new chat",
        leadingIcon: createElement(Icon, { name: "mic" }),
      },
      "Hello",
    ),
  );

  assert.match(markup, /^<button/);
  assert.match(markup, /type="button"/);
  assert.match(markup, /data-openbitfun-component="launcher-button"/);
  assert.match(markup, /data-openbitfun-part="icon"/);
  assert.match(markup, /data-openbitfun-name="mic"/);
  assert.match(markup, /data-openbitfun-part="label">Hello<\/span>/);
});

test("LauncherButton forwards native disabled state", () => {
  const markup = renderToStaticMarkup(
    createElement(LauncherButton, { disabled: true }, "Unavailable"),
  );

  assert.match(markup, /disabled=""/);
});

test("LauncherButton styles own reference state colors and shell-edge geometry", async () => {
  const styles = await readFile(
    new URL("../src/components/LauncherButton/LauncherButton.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /--openbitfun-control-launcher-button-min-inline-size/);
  assert.match(styles, /--openbitfun-control-launcher-button-block-size/);
  assert.match(styles, /--openbitfun-color-control-launcher-background/);
  assert.match(styles, /--openbitfun-color-control-launcher-background-hover/);
  assert.match(styles, /--openbitfun-color-control-launcher-background-pressed/);
  assert.match(styles, /--openbitfun-color-control-launcher-content-hover/);
  assert.match(styles, /--openbitfun-color-control-launcher-content-pressed/);
  assert.match(styles, /data-openbitfun-preview-state="hover"/);
  assert.match(styles, /data-openbitfun-preview-state="active"/);
});
