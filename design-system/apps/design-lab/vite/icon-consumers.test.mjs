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
