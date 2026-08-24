import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("public declarations do not expose internal wire DTOs", async () => {
  const names = ["client", "errors", "query", "session", "types"];
  const declarations = new Map(
    await Promise.all(
      names.map(async (name) => [
        name,
        await readFile(new URL(`../src/${name}.d.ts`, import.meta.url), "utf8"),
      ] as const),
    ),
  );

  for (const declaration of declarations.values()) {
    assert.doesNotMatch(declaration, /internal\/wire/);
  }

  assert.match(declarations.get("client") ?? "", /private constructor\(\)/);
  assert.match(declarations.get("query") ?? "", /private constructor\(\)/);
  assert.match(
    declarations.get("session") ?? "",
    /class Sessions[\s\S]*?private constructor\(\)/,
  );
  assert.match(
    declarations.get("session") ?? "",
    /class Session[\s\S]*?private constructor\(\)/,
  );
});

test("package files keep internal declarations and wire DTOs private", async () => {
  const packageJson = JSON.parse(
    await readFile(
      new URL("../../../../package.json", import.meta.url),
      "utf8",
    ),
  ) as { files?: string[] };

  assert.deepEqual(packageJson.files, [
    "dist/sdk/typescript/src/*.d.ts",
    "dist/sdk/typescript/src/*.js",
    "dist/sdk/typescript/src/internal/*.js",
    "dist/sdk/typescript/native/**",
    "dist/src/crates/adapters/transport/typescript/src/*.js",
    "README.md",
  ]);
});
