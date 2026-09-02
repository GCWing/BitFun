import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  Combobox,
  Field,
  FieldGroup,
  FieldRow,
  FormSection,
  Input,
  NumberInput,
  Select,
  Textarea,
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
      fieldSurface: "ambient",
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
  assert.match(markup, /data-field-surface="ambient"/);
  assert.match(markup, /data-bf-component="input"[^>]+data-field-surface="ambient"/);
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

test("field surface context reaches canonical field shells and resets for nested groups", () => {
  const controls = [
    ["input", createElement(Input)],
    ["select", createElement(Select, { options: [] })],
    ["combobox", createElement(Combobox, { options: [] })],
    ["textarea", createElement(Textarea)],
    ["number-input", createElement(NumberInput, { onValueChange() {}, value: 1 })],
  ];

  controls.forEach(([component, control]) => {
    const markup = renderToStaticMarkup(
      createElement(FieldGroup, { fieldSurface: "ambient" }, control),
    );
    assert.match(markup, new RegExp(`data-bf-component="${component}"[^>]+data-field-surface="ambient"`));
  });

  const nestedMarkup = renderToStaticMarkup(
    createElement(FieldGroup, { fieldSurface: "ambient" },
      createElement(FieldGroup, { fieldSurface: "default" }, createElement(Input)),
    ),
  );
  assert.match(nestedMarkup, /data-bf-component="input"[^>]+data-field-surface="default"/);
});

test("ambient field groups preserve opaque overlays while field shells reuse the group surface", async () => {
  const portalSource = await readFile(
    new URL("../src/overlay/Portal.tsx", import.meta.url),
    "utf8",
  );
  const componentStyles = await Promise.all([
    "Input/Input.module.css",
    "Select/Select.module.css",
    "Combobox/Combobox.module.css",
    "Textarea/Textarea.module.css",
    "NumberInput/NumberInput.module.css",
  ].map((path) => readFile(new URL(`../src/components/${path}`, import.meta.url), "utf8")));

  componentStyles.forEach((styles) => {
    assert.match(styles, /data-field-surface="ambient"/);
    assert.match(styles, /background:\s*transparent/);
  });
  assert.match(componentStyles[2], /\.popover\s*\{[\s\S]*?background:\s*var\(--bf-color-surface-panel\)/);
  assert.match(portalSource, /<FieldSurfaceContext\.Provider value="default">/);
});
