import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import ts from "typescript";

import { generateRuntimeValidators } from "./runtime-validator-generator.mjs";

test("generated validators enforce nested ts-rs wire structures", async () => {
  const directory = await mkdtemp(join(tmpdir(), "bitfun-wire-validator-"));
  try {
    await Promise.all([
      writeFile(join(directory, "Kind.ts"), 'export type Kind = "local" | "remote";\n'),
      writeFile(
        join(directory, "Child.ts"),
        "export type Child = { note?: string | null, tags: Array<string> };\n",
      ),
      writeFile(
        join(directory, "Envelope.ts"),
        [
          'import type { Child } from "./Child";',
          'import type { Kind } from "./Kind";',
          "export type Envelope = { kind: Kind, child?: Child, empty: Record<symbol, never> };",
          "",
        ].join("\n"),
      ),
    ]);

    const validators = await loadValidators(directory);

    assert.equal(
      validators.isEnvelope({
        kind: "local",
        child: { note: null, tags: ["one", "two"] },
        empty: {},
      }),
      true,
    );
    assert.equal(
      validators.isEnvelope({
        kind: "other",
        child: { tags: [] },
        empty: {},
      }),
      false,
    );
    assert.equal(
      validators.isEnvelope({
        kind: "remote",
        child: { tags: ["ok", 2] },
        empty: {},
      }),
      false,
    );
    assert.equal(
      validators.isEnvelope({
        kind: "remote",
        child: { tags: [], unexpected: true },
        empty: {},
      }),
      false,
    );
    assert.equal(
      validators.isEnvelope({ kind: "remote", empty: {}, unexpected: true }),
      false,
    );
    assert.equal(
      validators.isEnvelope({ kind: "remote", empty: { unexpected: true } }),
      false,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("generated validators reject missing required fields", async () => {
  const directory = await mkdtemp(join(tmpdir(), "bitfun-wire-validator-"));
  try {
    await writeFile(
      join(directory, "Required.ts"),
      "export type Required = { required: string, optional?: string };\n",
    );

    const validators = await loadValidators(directory);

    assert.equal(validators.isRequired({ required: "present" }), true);
    assert.equal(validators.isRequired({ optional: "present" }), false);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("generated validators accept JSON values without accepting arbitrary runtime values", async () => {
  const directory = await mkdtemp(join(tmpdir(), "bitfun-wire-validator-"));
  try {
    await writeFile(
      join(directory, "JsonPayload.ts"),
      "export type JsonPayload = { schema: Record<string, unknown>, value: unknown };\n",
    );

    const validators = await loadValidators(directory);

    assert.equal(
      validators.isJsonPayload({
        schema: { type: "object", properties: { summary: { type: "string" } } },
        value: { summary: "ready", count: 1, tags: [true, null] },
      }),
      true,
    );
    assert.equal(validators.isJsonPayload({ schema: [], value: "ready" }), false);
    assert.equal(validators.isJsonPayload({ schema: {}, value: undefined }), false);
    assert.equal(validators.isJsonPayload({ schema: {}, value: () => true }), false);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("generation fails closed for unsupported type syntax", async () => {
  const directory = await mkdtemp(join(tmpdir(), "bitfun-wire-validator-"));
  try {
    await writeFile(
      join(directory, "Unsupported.ts"),
      "export type Unsupported = { createdAt: Date };\n",
    );

    await assert.rejects(
      generateRuntimeValidators(directory),
      /unsupported type reference Date/i,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("generation rejects declarations outside the ts-rs type surface", async () => {
  const directory = await mkdtemp(join(tmpdir(), "bitfun-wire-validator-"));
  try {
    await writeFile(
      join(directory, "Unexpected.ts"),
      [
        "export type Unexpected = { value: string };",
        "export const runtimeValue = true;",
        "",
      ].join("\n"),
    );

    await assert.rejects(
      generateRuntimeValidators(directory),
      /unsupported declaration/i,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

async function loadValidators(directory) {
  const source = await generateRuntimeValidators(directory);
  const javascript = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
  }).outputText;
  return import(
    `data:text/javascript;base64,${Buffer.from(javascript).toString("base64")}`
  );
}
