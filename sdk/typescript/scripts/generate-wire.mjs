import { spawn } from "node:child_process";
import { mkdir, readdir, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { generateRuntimeValidators } from "./runtime-validator-generator.mjs";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(packageRoot, "../..");
const outputDirectory = join(packageRoot, "src", "internal", "wire");
const validatorOutput = join(
  packageRoot,
  "src",
  "internal",
  "wire-validators.ts",
);

await rm(outputDirectory, { recursive: true, force: true });
await rm(validatorOutput, { force: true });
await mkdir(outputDirectory, { recursive: true });

await run(
  "cargo",
  [
    "test",
    "--locked",
    "-p",
    "bitfun-sdk-host",
    "--no-default-features",
    "--features",
    "ts",
    "--lib",
    "export",
    "--",
    "--nocapture",
  ],
  {
    ...process.env,
    TS_RS_EXPORT_DIR: outputDirectory,
  },
);

const files = (await readdir(outputDirectory))
  .filter((file) => file.endsWith(".ts") && file !== "index.ts")
  .map((file) => file.slice(0, -3))
  .sort();
const requiredTypes = [
  "ErrorData",
  "HostCapabilities",
  "InitializeParams",
  "InitializeResult",
  "PermissionRespondParams",
  "PermissionRespondResult",
  "QueryCancelParams",
  "QueryCancelResult",
  "QueryEventParams",
  "QueryResultParams",
  "QueryStartParams",
  "QueryStartResult",
  "QueryUsage",
  "SessionCloseParams",
  "SessionCloseResult",
  "SessionCreateParams",
  "SessionCreateResult",
  "SessionResumeParams",
  "ShutdownResult",
  "TemporaryModelConfig",
  "TemporaryModelProvider",
];
const missing = requiredTypes.filter((type) => !files.includes(type));
if (missing.length > 0) {
  throw new Error(`SDK Host wire generation missed: ${missing.join(", ")}`);
}

await writeFile(
  validatorOutput,
  await generateRuntimeValidators(outputDirectory, {
    typeImportPath: "./wire/index.js",
  }),
);

const header = `// GENERATED CODE! DO NOT MODIFY BY HAND!\n// Source: bitfun-sdk-host protocol types via ts-rs.\n`;
const exports = files.map(
  (file) => `export type { ${file} } from "./${file}.js";`,
);
await writeFile(
  join(outputDirectory, "index.ts"),
  `${header}\n${exports.join("\n")}\n`,
);

async function run(command, args, env) {
  await new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command, args, {
      cwd: repositoryRoot,
      env,
      shell: false,
      stdio: "inherit",
    });
    child.once("error", rejectPromise);
    child.once("exit", (code, signal) => {
      if (code === 0) {
        resolvePromise();
        return;
      }
      rejectPromise(
        new Error(
          `${command} ${args.join(" ")} failed with ${signal ?? `exit ${code}`}`,
        ),
      );
    });
  });
}
