import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

// These text slots clip horizontally. Tight line boxes also clip the descenders
// of fallback fonts, so the slots (not icons or the entire control) own leading.
for (const [component, selector] of [
  ["ActionItem", ".label"],
  ["ActionCard", ".title"],
  ["ActionCard", ".description"],
  ["Button", ".label"],
  ["TabGroup", ".label"],
  ["SegmentedControl", ".label"],
  ["NavigationPanel", ".headingLabel"],
  ["Menu", ".headingLabel"],
  ["Dialog", ".title"],
  ["Disclosure", ".summary"],
]) {
  test(`${component} ${selector} retains ellipsis with font-safe line height`, async () => {
    const css = await readFile(new URL(
      `../src/components/${component}/${component}.module.css`, import.meta.url,
    ), "utf8");
    const declarations = {};
    for (const rule of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      if (!rule[1].split(",").some((entry) => entry.trim() === selector)) continue;
      for (const declaration of rule[2].split(";")) {
        const colon = declaration.indexOf(":");
        if (colon < 0) continue;
        declarations[declaration.slice(0, colon).trim()] = declaration.slice(colon + 1).trim();
      }
    }
    assert.equal(declarations["line-height"], "var(--bf-line-height-base)");
    assert.equal(declarations.overflow, "hidden");
    assert.equal(declarations["text-overflow"], "ellipsis");
    assert.equal(declarations["block-size"], undefined);
    assert.equal(declarations.height, undefined);
  });
}
