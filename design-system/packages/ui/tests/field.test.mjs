import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Field, Input } from "../dist/index.js";

test("Field associates label, description, and required state with its control", () => {
  const markup = renderToStaticMarkup(
    createElement(Field, {
      description: "Used in generated output",
      label: "Project name",
      required: true,
    }, createElement(Input, {
      "aria-describedby": "project-help",
      id: "project-name",
    })),
  );

  assert.match(markup, /data-bf-component="field"/);
  assert.match(markup, /data-orientation="vertical"/);
  assert.match(markup, /data-required="true"/);
  assert.match(markup, /<label[^>]+for="project-name"/);
  assert.match(markup, /data-bf-part="required"[^>]*>\*<\/span>/);
  assert.match(markup, /id="project-name-description"/);
  assert.match(markup, /aria-describedby="project-help project-name-description"/);
  assert.match(markup, /id="project-name"/);
  assert.match(markup, /required=""/);
});

test("Field exposes horizontal layout independently from its control", () => {
  const markup = renderToStaticMarkup(
    createElement(Field, {
      label: "Notifications",
      orientation: "horizontal",
    }, createElement("input", { type: "checkbox" })),
  );

  assert.match(markup, /data-orientation="horizontal"/);
  assert.match(markup, /data-bf-part="content"/);
  assert.match(markup, /data-bf-part="control"/);
  assert.match(markup, /type="checkbox"/);
});

test("Field styles consume shared content and typography tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-color-content-primary/);
  assert.match(styles, /--bf-color-content-muted/);
  assert.match(styles, /--bf-color-accent-default/);
  assert.match(styles, /--bf-font-size-caption/);
  assert.match(styles, /--bf-font-weight-semibold/);
});
