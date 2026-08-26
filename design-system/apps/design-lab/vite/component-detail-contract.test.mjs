import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const detailSource = new URL("../src/pages/ComponentDetailPage.tsx", import.meta.url);
const stylesSource = new URL("../src/styles.css", import.meta.url);

test("every preview matrix declares its state-column count", async () => {
  const source = await readFile(detailSource, "utf8");
  const matrices = source.match(/className="component-preview-matrix"/g) ?? [];
  const stateCounts = source.match(/data-state-count=\{states\.length\}/g) ?? [];

  assert.ok(matrices.length > 0);
  assert.equal(stateCounts.length, matrices.length);
});

test("preview matrices define horizontal columns for every registered state count", async () => {
  const source = await readFile(stylesSource, "utf8");

  assert.match(
    source,
    /\.component-preview-matrix\[data-state-count="1"\]\s*\{[^}]*grid-template-columns:\s*96px\s+minmax\(240px, 1fr\)/s,
  );
  assert.match(
    source,
    /\.component-preview-matrix\[data-state-count="4"\]\s*\{[^}]*grid-template-columns:\s*96px\s+repeat\(4, minmax\(124px, 1fr\)\)/s,
  );
  assert.match(
    source,
    /\.component-preview-matrix\[data-state-count="5"\]\s*\{[^}]*grid-template-columns:\s*96px\s+repeat\(5, minmax\(144px, 1fr\)\)/s,
  );
});

test("Button preview exposes the public presentation variants", async () => {
  const source = await readFile(detailSource, "utf8");
  const declaration = /const buttonVariants = \[([^\]]+)\] as const;/.exec(source);

  assert.ok(declaration);
  assert.deepEqual(
    [...declaration[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]),
    ["outline", "fill", "primary", "text"],
  );
});

test("Button preview opens on the filled variant used by the reference inspector", async () => {
  const source = await readFile(detailSource, "utf8");

  assert.match(
    source,
    /useState<\(typeof buttonVariants\)\[number\]>\("fill"\)/,
  );
});

test("Button matrix is limited to the four reference interaction states", async () => {
  const source = await readFile(detailSource, "utf8");
  const declaration = /case "Button":\s*case "IconButton":\s*return \[([^\]]+)\] as const;/.exec(source);

  assert.ok(declaration);
  assert.deepEqual(
    [...declaration[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]),
    ["default", "hover", "active", "disabled"],
  );
});

test("Button matrix uses the Session icon composition from the reference", async () => {
  const source = await readFile(detailSource, "utf8");

  assert.match(source, /MessageCircle/);
  assert.match(source, /ChevronDown/);
  assert.match(source, /components\.preview\.session/);
  assert.match(source, /state === "hover" \|\| state === "active"/);
});

test("Button inspector wires the real disabled, loading, and icon controls", async () => {
  const source = await readFile(detailSource, "utf8");

  assert.match(source, /setInspectorDisabled/);
  assert.match(source, /setInspectorLoading/);
  assert.match(source, /setPreviewIcon/);
  assert.match(source, /setPreviewIconPosition/);
  assert.match(source, /renderPreview\(previewState, variant, true\)/);
});

test("IconButton preview exposes its icon-only presentation contract", async () => {
  const source = await readFile(detailSource, "utf8");
  const declaration = /const iconButtonVariants = \[([^\]]+)\] as const;/.exec(source);

  assert.ok(declaration);
  assert.deepEqual(
    [...declaration[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]),
    ["quiet", "fill", "primary"],
  );
  assert.match(source, /data-component="icon-button"/);
  assert.match(source, /aria-label=\{t\("components\.preview\.listView"\)\}/);
  assert.match(source, /icon=\{<List aria-hidden="true" \/>\}/);
});

test("ActionItem preview keeps its trigger and end actions as separate contracts", async () => {
  const source = await readFile(detailSource, "utf8");

  assert.match(source, /component\.name === "ActionItem"/);
  assert.match(source, /leading=\{<MessageCircle aria-hidden="true" \/>\}/);
  assert.match(source, /shortcut=\{<KeyHint>K<\/KeyHint>\}/);
  assert.match(source, /id: "add"/);
  assert.match(source, /id: "more"/);
});

test("ActionItem preview reserves a full-width column for its complete anatomy", async () => {
  const source = await readFile(stylesSource, "utf8");

  assert.match(
    source,
    /\.component-preview-matrix\[data-component="action-item"\]\s*\{[^}]*grid-template-columns:\s*96px\s+repeat\(4, minmax\(280px, 1fr\)\)/s,
  );
  assert.match(
    source,
    /\.component-preview-matrix\[data-component="action-item"\]\s+\[data-bf-component="action-item"\]\s*\{[^}]*inline-size:\s*100%/s,
  );
});

test("Input, KeyHint, and SearchField previews expose composable slot and state contracts", async () => {
  const source = await readFile(detailSource, "utf8");

  assert.match(source, /case "Input":\s*case "SearchField":\s*return \["default", "hover", "focus-visible", "invalid", "disabled"\] as const/);
  assert.match(source, /component\.name === "Input"/);
  assert.match(source, /component\.name === "KeyHint"/);
  assert.match(source, /component\.name === "SearchField"/);
  assert.match(source, /trailing=\{<Eye aria-hidden="true" \/>\}/);
  assert.match(source, /leadingIcon=\{<SearchIcon aria-hidden="true" \/>\}/);
  assert.match(source, /shortcut=\{<KeyHint icon=\{<Command aria-hidden="true" \/>\}>K<\/KeyHint>\}/);
});

test("Field preview exposes label content independently from layout orientation", async () => {
  const source = await readFile(detailSource, "utf8");

  assert.match(source, /const fieldOrientations = \["vertical", "horizontal"\] as const/);
  assert.match(source, /description=\{t\("components\.preview\.fieldDescription"\)\}/);
  assert.match(source, /orientation=\{fieldOrientation\}/);
  assert.match(source, /component\.name === "Field"/);
});

test("PageHeader preview decouples semantic level from visual size and alignment", async () => {
  const source = await readFile(detailSource, "utf8");

  assert.match(source, /const pageHeaderAlignments = \["start", "center"\] as const/);
  assert.match(source, /const pageHeaderSizes = \["sm", "md", "lg", "display"\] as const/);
  assert.match(source, /level=\{2\}/);
  assert.match(source, /size=\{pageHeaderSize\}/);
  assert.match(source, /align=\{pageHeaderAlign\}/);
  assert.match(source, /action=\{\(/);
});

test("TabGroup preview carries the selected and outline reference composition", async () => {
  const source = await readFile(detailSource, "utf8");
  const declaration = /case "TabGroup":\s*return \[([^\]]+)\] as const;/.exec(source);

  assert.ok(declaration);
  assert.deepEqual(
    [...declaration[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]),
    ["selected", "unselected", "hover", "disabled"],
  );
  assert.match(source, /import \{ MessageCircle \} from "lucide-react"/);
  assert.match(source, /components\.preview\.welcome/);
  assert.match(source, /components\.preview\.settings/);
  assert.match(source, /data-component="tab-group"/);
  assert.match(source, /<TabGroup/);
});
