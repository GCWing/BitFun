import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  findForbiddenUiDependencies,
  findRawUiColors,
  readPublicCssVariableContract,
  validatePublishedPackageManifest,
} from "../src/contract.mjs";

const designSystemDirectory = path.resolve(
  fileURLToPath(new URL("../../../", import.meta.url)),
);
const uiSourceDirectory = path.join(designSystemDirectory, "packages", "ui", "src");

test("UI CSS consumes only generated public token variables", async () => {
  const contract = await readPublicCssVariableContract({
    systemCssPath: path.join(
      designSystemDirectory,
      "packages",
      "design-tokens",
      "dist",
      "tokens.css",
    ),
    themeCssPath: path.join(
      designSystemDirectory,
      "packages",
      "theme-bitfun",
      "dist",
      "themes.css",
    ),
    uiSourceDirectory,
  });

  assert.deepEqual(contract.missing, []);
});

test("UI source stays independent from application infrastructure", async () => {
  assert.deepEqual(await findForbiddenUiDependencies(uiSourceDirectory), []);
});

test("UI source does not introduce raw color values", async () => {
  assert.deepEqual(await findRawUiColors(uiSourceDirectory), []);
});

test("public package manifests expose only built artifacts", async () => {
  for (const packageName of ["design-tokens", "theme-bitfun", "ui"]) {
    const failures = await validatePublishedPackageManifest(
      path.join(designSystemDirectory, "packages", packageName, "package.json"),
    );
    assert.deepEqual(failures, [], `${packageName}: ${failures.join(", ")}`);
  }
});
