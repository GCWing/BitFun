import assert from "node:assert/strict";
import test from "node:test";
import {
  createTokenCatalog,
  diffResolvedTokens,
  mergeTokenDocuments,
  renderCss,
  resolveTokens,
  tokenNameToCssVariable,
} from "../src/index.mjs";

test("resolves aliases and emits stable CSS variable names", () => {
  const tokens = resolveTokens({
    ref: {
      accent: { $type: "color", $value: "#146c4b" },
    },
    color: {
      action: {
        primary: { $type: "color", $value: "{ref.accent}" },
      },
    },
  });

  assert.equal(tokens["color.action.primary"].value, "#146c4b");
  assert.equal(tokenNameToCssVariable("color.action.primary"), "--bf-color-action-primary");
  assert.match(renderCss(tokens), /--bf-color-action-primary: #146c4b;/);
});

test("can preserve exact and embedded references in generated CSS", () => {
  const tokens = resolveTokens({
    font: {
      size: {
        base: { $type: "dimension", $value: "14px" },
      },
    },
    type: {
      body: {
        fontSize: { $type: "dimension", $value: "{font.size.base}" },
        fluidSize: {
          $type: "dimension",
          $value: "clamp({font.size.base}, 2vw, 18px)",
        },
      },
    },
  });

  const css = renderCss(tokens, { preserveReferences: true });

  assert.equal(tokens["type.body.fontSize"].value, "14px");
  assert.match(css, /--bf-type-body-font-size: var\(--bf-font-size-base\);/);
  assert.match(
    css,
    /--bf-type-body-fluid-size: clamp\(var\(--bf-font-size-base\), 2vw, 18px\);/,
  );
});

test("merges token overrides without losing sibling groups", () => {
  const merged = mergeTokenDocuments(
    {
      size: {
        small: { $value: "28px" },
        medium: { $value: "34px" },
      },
    },
    {
      size: {
        medium: { $value: "32px" },
      },
    },
  );
  const tokens = resolveTokens(merged);

  assert.equal(tokens["size.small"].value, "28px");
  assert.equal(tokens["size.medium"].value, "32px");
});

test("reports circular aliases with the complete path", () => {
  assert.throws(
    () =>
      resolveTokens({
        first: { $value: "{second}" },
        second: { $value: "{first}" },
      }),
    /first -> second -> first/,
  );
});

test("returns only values changed by a mode", () => {
  const base = resolveTokens({
    control: {
      small: { $value: "28px" },
      medium: { $value: "34px" },
    },
  });
  const compact = resolveTokens({
    control: {
      small: { $value: "26px" },
      medium: { $value: "34px" },
    },
  });

  assert.deepEqual(Object.keys(diffResolvedTokens(base, compact)), ["control.small"]);
});

test("creates a mode-complete catalog for visual authoring consumers", () => {
  const comfortable = resolveTokens({
    space: {
      $type: "dimension",
      2: { $description: "Standard gap", $value: "8px" },
    },
  });
  const compact = resolveTokens({
    space: {
      $type: "dimension",
      2: { $value: "6px" },
    },
  });
  const catalog = createTokenCatalog(
    { comfortable, compact },
    { defaultMode: "comfortable" },
  );

  assert.deepEqual(catalog, [
    {
      category: "space",
      cssVariable: "--bf-space-2",
      description: "Standard gap",
      name: "space.2",
      type: "dimension",
      values: { comfortable: "8px", compact: "6px" },
    },
  ]);
});
