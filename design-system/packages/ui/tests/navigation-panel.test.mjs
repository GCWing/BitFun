import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  NavigationPanel,
  NavigationPanelItem,
  NavigationPanelSection,
  NavigationPanelSeparator,
} from "../dist/index.js";

test("NavigationPanel composes independent header, grouped body, and footer regions", () => {
  const markup = renderToStaticMarkup(
    createElement(
      NavigationPanel,
      {
        "aria-label": "Application navigation",
        footer: createElement("button", null, "Device"),
        header: createElement("button", null, "Search"),
        scrollbarVisibility: "always",
      },
      createElement(
        NavigationPanelSection,
        {
          actions: [{ icon: createElement("svg"), id: "add", label: "Add" }],
          title: "Sessions",
        },
        createElement(NavigationPanelItem, { selected: true }, "Welcome"),
        createElement(NavigationPanelItem, { disabled: true }, "Unavailable"),
      ),
      createElement(NavigationPanelSeparator),
    ),
  );

  assert.match(markup, /<nav[^>]+aria-label="Application navigation"/);
  assert.match(markup, /data-bf-component="navigation-panel"/);
  assert.match(markup, /data-bf-part="header"/);
  assert.match(markup, /data-bf-part="content"/);
  assert.match(markup, /data-bf-part="footer"/);
  assert.match(markup, /data-bf-scrollbar-visibility="always"/);
  assert.match(markup, /aria-labelledby="[^"]+"/);
  assert.match(markup, /aria-current="page"/);
  assert.match(markup, /disabled=""/);
  assert.match(markup, /role="separator"/);
  assert.match(markup, /aria-label="Add"/);
});

test("NavigationPanel styling owns only layout while reusing shared action and scrollbar contracts", async () => {
  const styles = await readFile(
    new URL("../src/components/NavigationPanel/NavigationPanel.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /--bf-layout-navigation-panel-inline-size/);
  assert.match(styles, /--bf-layout-navigation-panel-footer-height/);
  assert.match(styles, /--bf-color-surface-subtle/);
  assert.match(styles, /--bf-color-action-neutral-surface-pressed/);
  assert.match(styles, /aria-current/);
  assert.match(styles, /scrollbar-gutter: stable/);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});
