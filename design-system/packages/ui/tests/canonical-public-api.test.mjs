import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const componentSource = (name) => readFile(
  new URL(`../src/components/${name}/${name}.tsx`, import.meta.url),
  "utf8",
);

test("public component sizes use canonical names without compatibility aliases", async () => {
  const contracts = [
    ["Avatar", "AvatarSize"],
    ["Checkbox", "CheckboxSize"],
    ["Empty", "EmptyMediaSize"],
    ["Radio", "RadioSize"],
  ];

  for (const [component, typeName] of contracts) {
    const source = await componentSource(component);
    assert.match(source, new RegExp(`export type ${typeName} = "sm" \\| "md" \\| "lg";`));
    assert.doesNotMatch(source, /\b(?:small|medium|large)\b/);
  }
});

test("public semantic state axes do not retain retired aliases", async () => {
  const [alertSource, checkboxSource, textareaSource] = await Promise.all([
    componentSource("Alert"),
    componentSource("Checkbox"),
    componentSource("Textarea"),
  ]);

  assert.match(alertSource, /tone\?: AlertTone/);
  assert.doesNotMatch(alertSource, /type\?: AlertTone|tone \?\? type|resolvedTone/);
  assert.doesNotMatch(checkboxSource, /error\?: boolean|error \|\| invalid/);
  assert.doesNotMatch(textareaSource, /error\?: boolean|invalid \|\| error/);
});

test("value controls expose one canonical change event", async () => {
  const [numberInputSource, selectSource] = await Promise.all([
    componentSource("NumberInput"),
    componentSource("Select"),
  ]);

  assert.match(numberInputSource, /onValueChange: \(value: number\) => void/);
  assert.doesNotMatch(numberInputSource, /\bonChange\??:/);
  assert.match(selectSource, /onValueChange\?: \(value: SelectValue\) => void/);
  assert.doesNotMatch(selectSource, /\bonChange\??:/);
});
