import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const appSource = new URL("../src/App.tsx", import.meta.url);
const colorsSource = new URL("../src/pages/ColorsPage.tsx", import.meta.url);
const tokenWorkbenchSource = new URL(
  "../src/token-editor/TokenWorkbench.tsx",
  import.meta.url,
);

test("Colors is an independent Design Lab route backed by generated theme data", async () => {
  const [app, colors] = await Promise.all([
    readFile(appSource, "utf8"),
    readFile(colorsSource, "utf8"),
  ]);

  assert.match(app, /page: "colors"/);
  assert.match(app, /<ColorsPage/);
  assert.match(colors, /themeTokenCatalog/);
  assert.match(colors, /referenceColorScales/);
  assert.doesNotMatch(colors, /const\s+semanticColorTokens\s*=\s*\[/);
});

test("semantic color table compares all four published theme modes", async () => {
  const colors = await readFile(colorsSource, "utf8");

  for (const mode of [
    "values.light",
    "values.dark",
    "values.highContrastLight",
    "values.highContrastDark",
  ]) {
    assert.match(colors, new RegExp(mode.replace(".", "\\.")));
  }
});

test("Design Tokens workbench excludes color contracts after the split", async () => {
  const source = await readFile(tokenWorkbenchSource, "utf8");

  assert.match(source, /nonColorTokenCatalog\.filter/);
  assert.doesNotMatch(source, /<ReferenceColorPalette/);
});
