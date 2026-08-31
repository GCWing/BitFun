import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { IconButton } from "../dist/index.js";

test("IconButton requires an accessible label and keeps its icon decorative", () => {
  const markup = renderToStaticMarkup(
    createElement(IconButton, {
      "aria-label": "Show list",
      icon: createElement("svg", { "data-icon": "list" }),
    }),
  );

  assert.match(markup, /data-bf-component="icon-button"/);
  assert.match(markup, /data-bf-part="progress"/);
  assert.match(markup, /data-bf-part="icon"/);
  assert.match(markup, /aria-label="Show list"/);
  assert.match(markup, /aria-hidden="true"/);
  assert.match(markup, /data-icon="list"/);
  assert.match(markup, /data-bf-variant="quiet"/);
  assert.match(markup, /data-bf-shape="square"/);
  assert.match(markup, /type="button"/);
});

test("IconButton exposes presentation, shape, tone, and size independently", () => {
  const markup = renderToStaticMarkup(
    createElement(IconButton, {
      "aria-label": "Delete",
      icon: createElement("svg"),
      shape: "circle",
      size: "lg",
      tone: "danger",
      variant: "primary",
    }),
  );

  assert.match(markup, /data-bf-variant="primary"/);
  assert.match(markup, /data-bf-shape="circle"/);
  assert.match(markup, /data-bf-tone="danger"/);
  assert.match(markup, /data-size="lg"/);
});

test("IconButton exposes an extra-compact size for dense row actions", () => {
  const markup = renderToStaticMarkup(
    createElement(IconButton, {
      "aria-label": "Copy",
      icon: createElement("svg"),
      size: "xs",
    }),
  );

  assert.match(markup, /data-size="xs"/);
});

test("loading preserves the label while disabling activation", () => {
  const markup = renderToStaticMarkup(
    createElement(IconButton, {
      "aria-label": "Refresh",
      icon: createElement("svg"),
      loading: true,
    }),
  );

  assert.match(markup, /aria-busy="true"/);
  assert.match(markup, /disabled=""/);
  assert.match(markup, /data-loading="true"/);
});

test("IconButton styles consume shared action and geometry tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /\[data-bf-variant=primary\]/);
  assert.match(styles, /--bf-color-action-neutral-surface-hover/);
  assert.match(styles, /--bf-color-action-primary-background/);
  assert.match(styles, /--bf-control-height-sm/);
  assert.match(styles, /--bf-control-icon-button-xs-size/);
  assert.match(styles, /--bf-radius-sm/);
  assert.match(styles, /--bf-radius-pill/);
});
