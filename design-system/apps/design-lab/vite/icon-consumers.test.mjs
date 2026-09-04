import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Lab navigation, theme resources and export actions consume the shared icon catalog", async () => {
  const source = relative => readFile(new URL(`../src/${relative}`, import.meta.url), "utf8");
  const app = await source("App.tsx");
  assert.match(app, /name="palette"/);
  assert.match(app, /name="settings"/);
  assert.match(app, /icon: "palette"/);
  assert.doesNotMatch(app, /\b(?:Palette|Settings2)\b/);
  for (const page of ["ResourcesPage", "GettingStartedPage"]) {
    const markup = await source(`pages/${page}.tsx`);
    assert.match(markup, /icon: "palette"/);
    assert.match(markup, /<CatalogIcon name=\{Icon\}/);
    assert.doesNotMatch(markup, /\bPalette\b/);
  }
  const workbench = await source("token-editor/TokenWorkbench.tsx");
  assert.match(workbench, /<Icon name="arrow-down" size="sm"/);
  assert.doesNotMatch(workbench, /\bDownload\b/);
});

test("Mobile and Icon are standalone navigation items and component groups are collapsible", async () => {
  const source = relative => readFile(new URL(`../src/${relative}`, import.meta.url), "utf8");
  const [app, styles] = await Promise.all([
    source("App.tsx"),
    source("styles.css"),
  ]);

  assert.match(app, /component\.category !== "flow-chat"/);
  assert.match(app, /component\.category !== "mobile"/);
  assert.match(app, /component\.name !== "Icon"/);
  assert.match(app, /href="#mobile"/);
  assert.match(app, /category="mobile"/);
  assert.match(app, /href="#component\/icon"/);
  assert.match(app, /aria-controls="lab-standard-component-links"/);
  assert.match(app, /aria-controls="lab-flow-chat-component-links"/);
  assert.match(app, /hidden=\{!expandedComponentGroups\.components\}/);
  assert.match(app, /hidden=\{!expandedComponentGroups\["flow-chat"\]\}/);
  assert.match(styles, /\.lab-nav-group-chevron\[data-expanded\]/);
  assert.match(styles, /\.lab-component-links\[hidden\]/);
});

test("every published mobile component has catalog and detail previews", async () => {
  const source = relative => readFile(new URL(`../src/${relative}`, import.meta.url), "utf8");
  const [mobileEntry, catalog, detail, metadata] = await Promise.all([
    readFile(new URL("../../../packages/ui/src/mobile.ts", import.meta.url), "utf8"),
    source("pages/ComponentsPage.tsx"),
    source("pages/ComponentDetailPage.tsx"),
    source("i18n/componentMetadata.ts"),
  ]);
  const componentNames = [
    ...mobileEntry.matchAll(/^\s*(Mobile[A-Za-z]+),$/gm),
  ].map(match => match[1]);

  assert.ok(componentNames.length > 0);
  for (const componentName of componentNames) {
    assert.match(catalog, new RegExp(`case "${componentName}"`), `${componentName} lacks a catalog preview`);
    assert.match(detail, new RegExp(`component\\.name === "${componentName}"`), `${componentName} lacks a detail preview`);
    assert.match(metadata, new RegExp(`\\b${componentName}:`), `${componentName} lacks localized metadata`);
  }
});
