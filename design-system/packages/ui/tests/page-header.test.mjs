import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Button, PageHeader } from "../dist/index.js";

test("PageHeader decouples heading semantics from visual size", () => {
  const markup = renderToStaticMarkup(
    createElement(PageHeader, {
      description: "Interface language and visual appearance",
      level: 2,
      size: "display",
      title: "Appearance",
    }),
  );

  assert.match(markup, /data-openbitfun-component="page-header"/);
  assert.match(markup, /data-level="2"/);
  assert.match(markup, /data-size="display"/);
  assert.match(markup, /<h2[^>]*data-openbitfun-part="heading"[^>]*>Appearance<\/h2>/);
  assert.match(markup, /data-openbitfun-part="description">Interface language and visual appearance<\/span>/);
});

test("PageHeader exposes alignment and action content independently", () => {
  const markup = renderToStaticMarkup(
    createElement(PageHeader, {
      action: createElement(Button, null, "Close"),
      align: "center",
      leading: createElement("svg", { "aria-hidden": "true" }),
      title: "Good morning, coding partner",
    }),
  );

  assert.match(markup, /data-align="center"/);
  assert.match(markup, /data-openbitfun-part="leading"[^>]*><svg aria-hidden="true"><\/svg>/);
  assert.match(markup, /data-openbitfun-part="action"/);
  assert.match(markup, /<button/);
  assert.match(markup, /data-openbitfun-part="action"[^]*>Close<\/span>/);
});

test("PageHeader marks a required title with a decorative asterisk", () => {
  const markup = renderToStaticMarkup(
    createElement(PageHeader, {
      required: true,
      title: "Provider name",
    }),
  );

  assert.match(markup, /data-required="true"/);
  assert.match(
    markup,
    /data-openbitfun-part="heading"[^>]*>Provider name<span aria-hidden="true"[^>]*data-openbitfun-part="required"[^>]*>\*<\/span><\/h1>/,
  );
});

test("PageHeader omits the required marker by default", () => {
  const markup = renderToStaticMarkup(
    createElement(PageHeader, { title: "Appearance" }),
  );

  assert.match(markup, /data-required="false"/);
  assert.doesNotMatch(markup, /data-openbitfun-part="required"/);
});

test("PageHeader styles use shared typography and content tokens", async () => {
  const styles = await readFile(new URL("../src/components/PageHeader/PageHeader.module.css", import.meta.url), "utf8");

  assert.match(styles, /--openbitfun-type-heading-page-font-size/);
  assert.match(styles, /--openbitfun-type-heading-section-font-size/);
  assert.match(styles, /--openbitfun-type-heading-display-font-size/);
  assert.match(styles, /--openbitfun-type-display-sm-font-size/);
  assert.match(styles, /--openbitfun-type-body-lg-font-size/);
  assert.match(styles, /--openbitfun-color-content-primary/);
  assert.match(styles, /--openbitfun-color-content-muted/);
  assert.match(styles, /--openbitfun-color-content-required-indicator/);
});
