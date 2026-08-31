import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  Field,
  FieldGroup,
  FieldRow,
  FormSection,
  Input,
} from "../dist/index.js";

test("form grouping composes semantic sections, grouped surfaces, and independent rows", () => {
  const markup = renderToStaticMarkup(
    createElement(FormSection, {
      actions: createElement("button", { type: "button" }, "Reset"),
      description: "Connection settings",
      headingAs: "h3",
      leading: createElement("svg", { "aria-hidden": "true" }),
      title: "Provider",
    }, createElement(FieldGroup, {
      appearance: "subtle",
      dividers: true,
    },
    createElement(FieldRow, { align: "center", padding: "md" },
      createElement(Field, {
        controlWidth: "fill",
        label: "Name",
        labelWidth: "md",
        orientation: "horizontal",
      }, createElement(Input)),
    ),
    createElement(FieldRow, { align: "start", padding: "none" }, "Advanced"),
    )),
  );

  assert.match(markup, /<section[^>]+data-bf-component="form-section"/);
  assert.match(markup, /<h3[^>]+data-bf-part="title"[^>]*>Provider<\/h3>/);
  assert.match(markup, /data-bf-part="heading-region"/);
  assert.match(markup, /data-bf-part="leading"[^>]*><svg aria-hidden="true"><\/svg>/);
  assert.match(markup, /data-bf-part="description"[^>]*>Connection settings/);
  assert.match(markup, /data-bf-part="actions"[^>]*><button[^>]*>Reset/);
  assert.match(markup, /data-bf-component="field-group"/);
  assert.match(markup, /data-appearance="subtle"/);
  assert.match(markup, /data-dividers="true"/);
  assert.match(markup, /data-align="center"[^>]+data-bf-part="row"[^>]+data-padding="md"/);
  assert.match(markup, /data-align="start"[^>]+data-bf-part="row"[^>]+data-padding="none"/);
  assert.match(markup, /data-control-width="fill"/);
});

test("form grouping styles consume only shared public composition tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-layout-form-section-gap/);
  assert.match(styles, /--bf-layout-field-group-radius/);
  assert.match(styles, /--bf-layout-field-group-row-padding-block/);
  assert.match(styles, /--bf-color-surface-tertiary/);
  assert.match(styles, /--bf-color-border-subtle/);
});
