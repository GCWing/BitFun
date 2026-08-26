import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const detailSource = new URL("../src/pages/ComponentDetailPage.tsx", import.meta.url);

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
  const declaration = /case "Button":\s*return \[([^\]]+)\] as const;/.exec(source);

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
