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
  test(`${component} ${selector} uses shared overflow with font-safe line height`, async () => {
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
    assert.match(
      declarations["line-height"],
      component === "ActionItem"
        ? /^var\(--openbitfun-type-action-row-line-height\)$/
        : component === "ActionCard"
          ? selector === ".title"
            ? /^var\(--openbitfun-type-heading-section-line-height\)$/
            : /^var\(--openbitfun-type-action-row-line-height\)$/
        : component === "Dialog"
          ? /^var\(--openbitfun-type-heading-dialog-line-height\)$/
          : /^var\(--openbitfun-(?:line-height-base|type-body-sm-line-height|type-label-(?:xs|md)-line-height)\)$/,
    );
    assert.equal(declarations.overflow, "hidden");
    assert.equal(declarations["text-overflow"], undefined);
    assert.equal(declarations["block-size"], undefined);
    assert.equal(declarations.height, undefined);
    const source = await readFile(new URL(
      `../src/components/${component}/${component}.tsx`, import.meta.url,
    ), "utf8");
    assert.match(source, /<OverflowText\b/);
  });
}
