import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const detailSource = new URL("../src/pages/ComponentDetailPage.tsx", import.meta.url);
const stylesSource = new URL("../src/styles.css", import.meta.url);

test("Button preview exposes the three API variants", async () => {
  const source = await readFile(detailSource, "utf8");
  const declaration = /const buttonVariants = \[([^\]]+)\] as const satisfies readonly ButtonVariant\[\];/.exec(source);

  assert.ok(declaration);
  assert.deepEqual(
    [...declaration[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]),
    ["outline", "fill", "text"],
  );
});

test("Button matrix maps the four supported presentations onto variant and tone", async () => {
  const source = await readFile(detailSource, "utf8");
  const styles = await readFile(stylesSource, "utf8");

  assert.match(source, /label: "outline", tone: "neutral", variant: "outline"/);
  assert.match(source, /label: "fill", tone: "neutral", variant: "fill"/);
  assert.match(source, /label: "main", tone: "primary", variant: "fill"/);
  assert.match(source, /label: "text", tone: "primary", variant: "text"/);
  assert.match(styles, /grid-template-rows: 48px repeat\(4, 102px\)/);
  assert.match(styles, /:not\(\[data-bf-variant="text"\]\)/);
});

test("Button preview opens on the filled variant used by the reference inspector", async () => {
  const source = await readFile(detailSource, "utf8");

  assert.match(
    source,
    /useState<ButtonVariant>\("fill"\)/,
  );
});

test("Button and IconButton matrices use the four reference interaction states", async () => {
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
  assert.match(source, /renderPreview\(previewState, variant, tone, true\)/);
});

test("IconButton preview exposes interaction states and accessible sample code", async () => {
  const source = await readFile(detailSource, "utf8");
  const styles = await readFile(stylesSource, "utf8");

  assert.match(source, /component\.name === "IconButton"/);
  assert.match(source, /data-component="icon-button"/);
  assert.match(source, /aria-label=\{t\("components\.preview\.moreActions"\)\}/);
  assert.match(source, /<List aria-hidden="true"/);
  assert.match(styles, /\[data-component="icon-button"\]/);
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

test("component foundations remain visible as state matrices in Design Lab", async () => {
  const source = await readFile(detailSource, "utf8");
  const styles = await readFile(stylesSource, "utf8");

  for (const component of [
    "heading",
    "input",
    "search",
    "key-hint",
    "list-item",
    "media-thumbnail",
    "navigation-list",
    "prompt-composer",
  ]) {
    assert.match(source, new RegExp(`data-component="${component}"`));
    assert.match(styles, new RegExp(`data-component="${component}"`));
  }

  assert.match(source, /homepagePreviewImage/);
  assert.match(source, /macbookAirPreviewImage/);
  assert.match(source, /placeholderDeviceImage/);
  assert.match(source, /placeholderServerImage/);
});

test("chat input maps to PromptComposer instead of overloading native Input", async () => {
  const source = await readFile(detailSource, "utf8");

  assert.match(source, /component\.name === "PromptComposer"/);
  assert.match(source, /<PromptComposer/);
  assert.match(source, /startControls=/);
  assert.match(source, /endControls=/);
  assert.match(source, /How can I help you/);
});

test("normal, search, and chat input scenes have separate public previews", async () => {
  const source = await readFile(detailSource, "utf8");

  assert.match(source, /component\.name === "Input"/);
  assert.match(source, /component\.name === "Search"/);
  assert.match(source, /component\.name === "PromptComposer"/);
  assert.match(source, /defaultValue="BitFun is an AI-driven programming environment\."/);
  assert.match(source, /<Search/);
  assert.doesNotMatch(source, /variant="search"/);
});
