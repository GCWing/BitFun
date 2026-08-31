import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  NavigationPanel,
  NavigationPanelBody,
  NavigationPanelContent,
  NavigationPanelFooter,
  NavigationPanelHeader,
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
      },
      createElement(NavigationPanelHeader, null, createElement("button", null, "Search")),
      createElement(
        NavigationPanelBody,
        { scrollbarVisibility: "always" },
        createElement(
          NavigationPanelContent,
          null,
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
      ),
      createElement(NavigationPanelFooter, null, createElement("button", null, "Device")),
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

test("NavigationPanel styling reuses shared action and scrollbar contracts", async () => {
  const styles = await readFile(
    new URL("../src/components/NavigationPanel/NavigationPanel.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /--bf-layout-navigation-panel-inline-size/);
  assert.match(styles, /--bf-layout-navigation-panel-footer-height/);
  assert.match(styles, /\.items\s*\{[^}]*gap: calc\(var\(--bf-space-1\) \/ 2\)/);
  assert.match(styles, /--bf-color-surface-chrome/);
  assert.match(styles, /--bf-color-selection-surface/);
  assert.match(styles, /aria-current/);
  assert.match(styles, /scrollbar-gutter: stable/);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});

test("NavigationPanel separates group captions, destinations, and the selected destination", async () => {
  const styles = await readFile(
    new URL("../src/components/NavigationPanel/NavigationPanel.module.css", import.meta.url),
    "utf8",
  );
  const actionStyles = await readFile(
    new URL("../src/components/ActionItem/ActionItem.module.css", import.meta.url),
    "utf8",
  );
  const heading = styles.match(/\.headingLabel\s*\{([^}]+)\}/)?.[1];
  assert.ok(heading);
  assert.match(heading, /color: color-mix\(in srgb, var\(--bf-color-content-primary\) 50%, transparent\)/);
  assert.match(heading, /font-family: var\(--bf-type-meta-font-family\)/);
  assert.match(heading, /font-size: var\(--bf-type-meta-font-size\)/);
  assert.match(heading, /font-weight: var\(--bf-type-meta-font-weight\)/);
  assert.match(heading, /line-height: var\(--bf-line-height-base\)/);
  assert.match(styles, /\.item\[data-bf-tone="neutral"\]:not\(\[data-disabled="true"\]\)\s*\{\s*color: var\(--bf-color-content-primary\)/);
  assert.match(actionStyles, /\.label\s*\{[^}]*font-size: var\(--bf-font-size-sm\)/);
  assert.match(actionStyles, /\.label\s*\{[^}]*font-weight: var\(--bf-font-weight-regular\)/);
  assert.match(styles, /\.item > \[data-bf-part="trigger"\]\[aria-current\] > \[data-bf-part="label"\]\s*\{\s*font-weight: var\(--bf-font-weight-semibold\)/);
  assert.match(styles, /@media \(prefers-contrast: more\)[\s\S]*?color: var\(--bf-color-content-muted\)/);
  assert.match(styles, /:global\(\[data-contrast="high"\]\) \.headingLabel\s*\{\s*color: var\(--bf-color-content-muted\)/);
  assert.match(styles, /@media \(forced-colors: active\)[\s\S]*?color: CanvasText/);
});
