import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { TabGroup } from "../dist/index.js";

const items = [
  {
    icon: createElement("svg", { "data-icon": "welcome" }),
    label: "Welcome",
    panelId: "welcome-panel",
    value: "welcome",
  },
  {
    icon: createElement("svg", { "data-icon": "settings" }),
    label: "Settings",
    panelId: "settings-panel",
    value: "settings",
  },
];

test("TabGroup exposes a single selected tab with native button behavior", () => {
  const markup = renderToStaticMarkup(
    createElement(TabGroup, {
      "aria-label": "Workspace views",
      defaultValue: "welcome",
      items,
    }),
  );

  assert.match(markup, /data-bf-component="tab-group"/);
  assert.match(markup, /role="tablist"/);
  assert.match(markup, /aria-orientation="horizontal"/);
  assert.equal((markup.match(/role="tab"/g) ?? []).length, 2);
  assert.equal((markup.match(/aria-selected="true"/g) ?? []).length, 1);
  assert.match(markup, /aria-controls="welcome-panel"/);
  assert.match(markup, /tabindex="0"/);
  assert.match(markup, /tabindex="-1"/);
  assert.equal((markup.match(/type="button"/g) ?? []).length, 2);
});

test("controlled value and disabled items preserve selection and focus contracts", () => {
  const markup = renderToStaticMarkup(
    createElement(TabGroup, {
      "aria-label": "Workspace views",
      items: [items[0], { ...items[1], disabled: true }],
      value: "welcome",
    }),
  );

  assert.match(markup, /Welcome<\/span><\/button>/);
  assert.match(markup, /aria-disabled="true"/);
  assert.match(markup, /disabled=""/);
  assert.equal((markup.match(/aria-selected="true"/g) ?? []).length, 1);
});

test("icons are decorative and labels remain accessible", () => {
  const markup = renderToStaticMarkup(
    createElement(TabGroup, { "aria-label": "Workspace views", items }),
  );

  assert.equal((markup.match(/aria-hidden="true"/g) ?? []).length, 2);
  assert.match(markup, /data-icon="welcome"/);
  assert.match(markup, />Welcome<\/span>/);
  assert.match(markup, />Settings<\/span>/);
});

test("end actions are rendered beside tabs instead of nesting interactive controls", () => {
  const markup = renderToStaticMarkup(
    createElement(TabGroup, {
      "aria-label": "Workspace views",
      items: [
        items[0],
        {
          ...items[1],
          endAction: createElement(
            "button",
            { "aria-label": "Close Settings", type: "button" },
            "Close",
          ),
        },
      ],
    }),
  );

  assert.equal((markup.match(/data-bf-part="item"/g) ?? []).length, 2);
  assert.equal((markup.match(/data-has-end-action="true"/g) ?? []).length, 1);
  assert.match(markup, /data-bf-part="endAction"/);
  assert.match(markup, /<\/button><span[^>]+data-bf-part="endAction"><button/);
  assert.equal((markup.match(/type="button"/g) ?? []).length, 3);
});

test("TabGroup styling uses its geometry contract and Button semantic colors", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-control-tab-group-gap/);
  assert.match(styles, /--bf-control-tab-group-item-gap/);
  assert.match(styles, /--bf-control-tab-group-item-height/);
  assert.match(styles, /--bf-control-tab-group-item-icon-size/);
  assert.match(styles, /--bf-control-tab-group-item-padding-inline/);
  assert.match(styles, /--bf-control-tab-group-item-action-size/);
  assert.match(styles, /--bf-control-tab-group-item-action-inset/);
  assert.match(styles, /--bf-control-tab-group-item-radius/);
  assert.match(styles, /--bf-color-action-neutral-border/);
  assert.match(styles, /--bf-color-action-neutral-content/);
  assert.match(styles, /--bf-color-action-neutral-surface/);
  assert.match(styles, /--bf-font-weight-regular/);
  assert.match(styles, /--bf-font-weight-semibold/);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});

test("TabGroup implements wrapped arrow, Home, and End navigation", async () => {
  const source = await readFile(
    new URL("../src/components/TabGroup/TabGroup.tsx", import.meta.url),
    "utf8",
  );

  assert.match(source, /ArrowRight/);
  assert.match(source, /ArrowLeft/);
  assert.match(source, /event\.key === "Home"/);
  assert.match(source, /event\.key === "End"/);
  assert.match(source, /enabledItems\.length/);
  assert.match(source, /onValueChange\?\.\(item\.value\)/);
});
