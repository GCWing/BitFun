import assert from "node:assert/strict";
import test from "node:test";
import { componentRegistry } from "../dist/registry.js";

test("component names remain unique", () => {
  const names = componentRegistry.map((component) => component.name);
  assert.equal(new Set(names).size, names.length);
});

test("registry exposes only the formal stable components", () => {
  assert.deepEqual(
    componentRegistry.map((component) => component.name),
    ["Button", "Field", "IconButton", "Input", "KeyHint", "SearchField", "Switch", "TabGroup"],
  );
  assert.equal(
    componentRegistry.every((component) => component.maturity === "stable"),
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
          token.startsWith("control.") ||
          token.startsWith("font.") ||
          token.startsWith("radius."),
      ),
      true,
      `${component.name} contains a token outside the allowed public layers.`,
    );
  }
});
