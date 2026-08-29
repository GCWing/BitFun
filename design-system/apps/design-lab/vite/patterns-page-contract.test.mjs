import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const appSource = new URL("../src/App.tsx", import.meta.url);
const pageSource = new URL("../src/pages/PatternsPage.tsx", import.meta.url);
const stylesSource = new URL("../src/styles.css", import.meta.url);

test("Patterns is a first-class searchable Design Lab route", async () => {
  const app = await readFile(appSource, "utf8");
  assert.match(app, /page: "patterns"/);
  assert.match(app, /<PatternsPage/);
  assert.match(app, /search\.patternsDetail/);
  assert.match(app, /nav\.patterns/);
});

test("Patterns composes only public non-FlowChat contracts", async () => {
  const page = await readFile(pageSource, "utf8");
  assert.match(page, /data-bf-pattern="settings-form"/);
  assert.match(page, /data-bf-pattern="navigation-panel"/);
  assert.match(page, /data-bf-pattern="search-command-surface"/);
  assert.match(page, /data-bf-pattern="device-card"/);
  assert.match(page, /<Disclosure/);
  assert.match(page, /<FieldGroup/);
  assert.match(page, /<NavigationPanel/);
  assert.match(page, /<SearchField/);
  assert.match(page, /<ActionCard/);
  assert.match(page, /<StatusPill/);
  assert.doesNotMatch(page, /FlowChat|ChatComposer|<Composer/);
});

test("Patterns remain responsive and consume theme contracts", async () => {
  const styles = await readFile(stylesSource, "utf8");
  assert.match(styles, /\.pattern-navigation-stage\s*\{[^}]*grid-template-columns:\s*216px minmax\(0, 1fr\)/s);
  assert.match(styles, /\.pattern-action-grid\s*\{[^}]*grid-template-columns:\s*repeat\(2, minmax\(0, 1fr\)\)/s);
  assert.match(styles, /@media \(max-width: 640px\)[\s\S]*\.pattern-action-grid\s*\{[^}]*grid-template-columns:\s*1fr/s);
  assert.match(styles, /\.pattern-settings \[data-bf-component="field"\]\[data-orientation="horizontal"\]\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/s);
  assert.doesNotMatch(styles.match(/\/\* Patterns \*\/[\s\S]*?\/\* Component reference page \*\//)?.[0] ?? "", /#[0-9a-f]{3,8}/i);
});
