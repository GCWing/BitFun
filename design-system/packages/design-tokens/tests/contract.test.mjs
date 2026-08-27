import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  diffResolvedTokens,
  mergeTokenDocuments,
  resolveTokens,
} from "@bitfun/token-engine";
import {
  tokenCatalog,
  tokenModes,
  tokens,
} from "../dist/index.js";

const packageDirectory = fileURLToPath(new URL("../", import.meta.url));

async function readSource(fileName) {
  return JSON.parse(
    await readFile(path.join(packageDirectory, "src", fileName), "utf8"),
  );
}

test("system tokens remain color and brand independent", async () => {
  const systemTokens = resolveTokens(await readSource("system.tokens.json"));
  assert.equal(
    Object.keys(systemTokens).some((name) => name.startsWith("color.")),
    false,
  );
});

test("Switch geometry preserves the compact reference contract", () => {
  assert.equal(tokens["control.switch.trackWidth"], "28px");
  assert.equal(tokens["control.switch.trackHeight"], "16px");
  assert.equal(tokens["control.switch.thumbSize"], "12px");
  assert.equal(tokens["control.switch.thumbInset"], "2px");
  assert.equal(tokens["control.switch.thumbTravel"], "12px");
  assert.equal(tokens["control.switch.thumbTravelReverse"], "-12px");
});

test("TabGroup geometry preserves the capsule selected and outline contract", async () => {
  const systemDocument = await readSource("system.tokens.json");

  assert.equal(tokens["control.tabGroup.gap"], "8px");
  assert.equal(tokens["control.tabGroup.itemGap"], "6px");
  assert.equal(tokens["control.tabGroup.itemHeight"], "40px");
  assert.equal(tokens["control.tabGroup.itemIconSize"], "16px");
  assert.equal(tokens["control.tabGroup.itemPaddingInline"], "16px");
  assert.equal(tokens["control.tabGroup.itemActionSize"], "20px");
  assert.equal(tokens["control.tabGroup.itemActionInset"], "8px");
  assert.equal(systemDocument.control.tabGroup.itemRadius.$value, "{radius.pill}");
  assert.equal(tokens["control.tabGroup.itemRadius"], "9999px");
});

test("split-view content panels preserve the elevated shell curvature contract", async () => {
  const systemDocument = await readSource("system.tokens.json");

  assert.equal(
    systemDocument.layout.splitView.contentPanelRadius.$value,
    "{radius.3xl}",
  );
  assert.equal(tokens["radius.3xl"], "24px");
  assert.equal(tokens["layout.splitView.contentPanelRadius"], "24px");
});

test("shared scrollbar geometry preserves the compact native scrollbar contract", async () => {
  const systemDocument = await readSource("system.tokens.json");

  assert.equal(tokens["scrollbar.width"], "6px");
  assert.equal(systemDocument.scrollbar.radius.$value, "{radius.pill}");
  assert.equal(tokens["scrollbar.radius"], "9999px");
});

test("Modal tokens preserve the reference surface and chrome contract", async () => {
  const systemDocument = await readSource("system.tokens.json");

  assert.equal(tokens["overlay.modal.backdropBlur"], "blur(40px)");
  assert.equal(tokens["overlay.modal.surfaceRadius"], "28px");
  assert.equal(tokens["overlay.modal.headerGap"], "20px");
  assert.equal(tokens["overlay.modal.headerPaddingBlockStart"], "24px");
  assert.equal(tokens["overlay.modal.headerPaddingBlockEnd"], "20px");
  assert.equal(tokens["overlay.modal.headerPaddingInline"], "24px");
  assert.equal(tokens["overlay.modal.titleFontSize"], "24px");
  assert.equal(tokens["overlay.modal.titleFontWeight"], 700);
  assert.equal(systemDocument.overlay.modal.scrollbarWidth.$value, "{scrollbar.width}");
  assert.equal(tokens["overlay.modal.scrollbarWidth"], "6px");
  assert.equal(tokens["overlay.modal.footerBlur"], "blur(20px)");
  assert.equal(tokens["overlay.modal.footerFadeExtent"], "24px");
  assert.equal(tokens["overlay.modal.footerContentInset"], "104px");
});

test("control heights preserve an eight-pixel size step in every density mode", () => {
  const valuesFor = (name) => tokenCatalog.find((token) => token.name === name)?.values;

  assert.deepEqual(valuesFor("control.height.sm"), {
    comfortable: "32px",
    compact: "28px",
    touch: "40px",
  });
  assert.deepEqual(valuesFor("control.height.md"), {
    comfortable: "40px",
    compact: "36px",
    touch: "48px",
  });
  assert.deepEqual(valuesFor("control.height.lg"), {
    comfortable: "48px",
    compact: "44px",
    touch: "56px",
  });
  assert.deepEqual(valuesFor("control.hitTarget"), {
    comfortable: "40px",
    compact: "36px",
    touch: "48px",
  });
  assert.deepEqual(valuesFor("control.tabGroup.itemHeight"), {
    comfortable: "40px",
    compact: "36px",
    touch: "48px",
  });
});

test("shared system scales preserve the migrated Web UI foundation contract", () => {
  assert.deepEqual(
    Object.fromEntries([
      "space.1",
      "space.2",
      "space.3",
      "space.4",
      "space.5",
      "space.6",
      "space.8",
      "space.10",
      "space.12",
      "space.16",
    ].map((name) => [name, tokens[name]])),
    {
      "space.1": "4px",
      "space.2": "8px",
      "space.3": "12px",
      "space.4": "16px",
      "space.5": "20px",
      "space.6": "24px",
      "space.8": "32px",
      "space.10": "40px",
      "space.12": "48px",
      "space.16": "64px",
    },
  );
  assert.equal(tokens["font.family.control"].startsWith("'SF Pro Text'"), true);
  assert.equal(tokens["font.family.control"].includes("'Noto Sans SC'"), true);
  assert.equal(tokens["font.family.sans"].startsWith("'Noto Sans SC'"), true);
  assert.equal(tokens["font.family.mono"].startsWith("'JetBrains Mono'"), true);
  assert.equal(tokens["font.size.micro"], "10px");
  assert.equal(tokens["font.size.xs"], "12px");
  assert.equal(tokens["font.size.small"], "13px");
  assert.equal(tokens["font.size.4xl"], "26px");
  assert.equal(tokens["font.weight.regular"], 400);
  assert.equal(tokens["font.weight.bold"], 600);
  assert.equal(tokens["lineHeight.base"], 1.5);
  assert.equal(tokens["radius.xs"], "4px");
  assert.equal(tokens["radius.sm"], "6px");
  assert.equal(tokens["radius.2xl"], "20px");
  assert.equal(tokens["radius.3xl"], "24px");
  assert.equal(tokens["motion.duration.instant"], "80ms");
  assert.equal(tokens["motion.duration.slow"], "420ms");
  assert.equal(tokens["motion.easing.standard"], "cubic-bezier(0.23, 1, 0.32, 1)");
  assert.equal(tokens["layer.modal"], 200);
  assert.equal(tokens["layer.contextMenu"], 500);
});

test("component spacing remains available in every density mode", () => {
  const inlineSpacing = tokenCatalog.find(
    ({ name }) => name === "space.component.inline",
  );
  const blockSpacing = tokenCatalog.find(
    ({ name }) => name === "space.component.block",
  );

  assert.deepEqual(inlineSpacing?.values, {
    comfortable: "12px",
    compact: "10px",
    touch: "16px",
  });
  assert.deepEqual(blockSpacing?.values, {
    comfortable: "8px",
    compact: "6px",
    touch: "12px",
  });
});

test("density documents only override existing system tokens", async () => {
  const systemDocument = await readSource("system.tokens.json");
  const compactDocument = await readSource("density-compact.tokens.json");
  const baseTokens = resolveTokens(systemDocument);
  const compactTokens = resolveTokens(
    mergeTokenDocuments(systemDocument, compactDocument),
  );
  const changes = diffResolvedTokens(baseTokens, compactTokens);

  for (const name of Object.keys(changes)) {
    assert.ok(name in baseTokens, `Density mode introduced unknown token ${name}.`);
  }
});

test("public token catalog exposes every system token in every density mode", () => {
  assert.equal(tokenCatalog.length, Object.keys(tokens).length);
  assert.deepEqual(tokenModes, ["comfortable", "compact", "touch"]);
  for (const token of tokenCatalog) {
    assert.equal(token.cssVariable.startsWith("--bf-"), true);
    assert.deepEqual(Object.keys(token.values), tokenModes);
  }
});
