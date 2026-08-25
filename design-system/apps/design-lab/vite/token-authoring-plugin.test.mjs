import assert from "node:assert/strict";
import test from "node:test";
import {
  coerceTokenValue,
  setTokenDocumentValue,
} from "./token-authoring-plugin.mjs";

test("coerces persisted token values according to their contract type", () => {
  assert.equal(coerceTokenValue("color", "#ABCDEF"), "#abcdef");
  assert.equal(coerceTokenValue("dimension", "12px"), "12px");
  assert.equal(coerceTokenValue("number", "1.5"), 1.5);
  assert.equal(coerceTokenValue("fontWeight", "600"), 600);
  assert.throws(() => coerceTokenValue("color", "red"), /six-digit hexadecimal/);
  assert.throws(() => coerceTokenValue("dimension", "12"), /require px/);
});

test("writes a mode override without replacing sibling token definitions", () => {
  const document = {
    control: {
      height: {
        sm: { $type: "dimension", $value: "26px" },
      },
    },
  };

  setTokenDocumentValue(document, "control.height.md", "dimension", "30px");

  assert.deepEqual(document.control.height.sm, {
    $type: "dimension",
    $value: "26px",
  });
  assert.deepEqual(document.control.height.md, {
    $type: "dimension",
    $value: "30px",
  });
});

