import assert from "node:assert/strict";
import test from "node:test";
import { componentRegistry, figmaSourceInventory } from "../dist/registry.js";

test("component names remain unique", () => {
  const names = componentRegistry.map((component) => component.name);
  assert.equal(new Set(names).size, names.length);
});

test("registry exposes only the formal stable components", () => {
  assert.deepEqual(
    componentRegistry.map((component) => component.name),
    ["Button", "IconButton", "Heading", "Input", "KeyHint", "ListItem", "MediaThumbnail", "NavigationList", "PromptComposer", "Switch", "TabGroup"],
  );
  assert.equal(
    componentRegistry.every((component) => component.maturity === "stable"),
    true,
  );
});

test("every registered component records its remote Figma source", () => {
  for (const component of componentRegistry) {
    assert.equal(component.figma.fileKey, "5k2waflRzrRdd8yLYEOam8");
    assert.ok(component.figma.nodeIds.length > 0, `${component.name} has no Figma node id.`);
    assert.equal(component.figma.nodeIds.length, component.figma.sourceNames.length);
  }
});

test("remote Figma component-set inventory has an explicit local disposition", () => {
  const expectedNodeIds = [
    "2:22",
    "2:130",
    "4:5332",
    "4:5355",
    "61:568",
    "61:741",
    "66:2171",
    "98:966",
    "98:1062",
    "131:6340",
    "133:6763",
    "142:11871",
    "142:12284",
    "149:12404",
    "149:13825",
  ];

  assert.deepEqual(
    [...new Set(figmaSourceInventory.map((source) => source.nodeId))].sort(),
    expectedNodeIds.sort(),
  );
  assert.equal(
    figmaSourceInventory.every((source) => source.disposition === "implemented" || source.reason),
    true,
  );
});

test("every registered component declares states and owned tokens", () => {
  for (const component of componentRegistry) {
    assert.ok(component.states.length > 0, `${component.name} has no declared states.`);
    assert.ok(component.tokens.length > 0, `${component.name} has no declared tokens.`);
    assert.equal(
      component.tokens.every(
        (token) =>
          token.startsWith("color.") ||
          token.startsWith("border.") ||
          token.startsWith("control.") ||
          token.startsWith("font.") ||
          token.startsWith("radius.") ||
          token.startsWith("shadow."),
      ),
      true,
      `${component.name} contains a token outside the allowed public layers.`,
    );
  }
});
