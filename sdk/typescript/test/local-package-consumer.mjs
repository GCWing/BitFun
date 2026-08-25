import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const isolatedRoot = await mkdtemp(join(tmpdir(), "bitfun-sdk-consumer-"));
const packedRoot = join(isolatedRoot, "packed");
const consumerRoot = join(isolatedRoot, "consumer");
const workspace = join(isolatedRoot, "workspace");
const userRoot = join(isolatedRoot, "user-root");
const configRoot = join(isolatedRoot, "config-root");

try {
  await Promise.all(
    [packedRoot, consumerRoot, workspace, userRoot, configRoot].map((path) =>
      mkdir(path, { recursive: true }),
    ),
  );
  const packed = await runNpm([
    "pack",
    packageRoot,
    "--pack-destination",
    packedRoot,
    "--json",
  ], isolatedRoot);
  const packResult = JSON.parse(packed.stdout);
  assert.equal(Array.isArray(packResult), true);
  assert.equal(packResult.length, 1);
  const tarball = join(packedRoot, packResult[0].filename);

  await writeFile(
    join(consumerRoot, "package.json"),
    `${JSON.stringify({ private: true, type: "module" }, null, 2)}\n`,
  );
  await runNpm(
    ["install", "--ignore-scripts", "--no-audit", "--no-fund", tarball],
    consumerRoot,
  );
  await writeFile(
    join(consumerRoot, "run.mjs"),
    `
import assert from "node:assert/strict";
import { AgentClient } from "@bitfun/agent-sdk";

const client = await AgentClient.start({
  cwd: process.cwd(),
  model: {
    provider: "openai",
    model: "fixture-model",
    apiKey: "local-consumer-fixture",
    baseUrl: "http://127.0.0.1:9/v1",
  },
});
try {
  assert.equal(client.capabilities.query, true);
  assert.equal(client.capabilities.toolEvents, true);
  assert.equal(client.capabilities.imageInput, true);
  assert.equal(client.capabilities.usage, true);
  assert.equal(client.capabilities.permissionResponses, true);
} finally {
  await client.close();
}
process.stdout.write("local-package-consumer: PASS\\n");
`,
  );

  const result = await run(process.execPath, [join(consumerRoot, "run.mjs")], consumerRoot, {
    ...process.env,
    BITFUN_E2E_STORAGE_GUARD: "1",
    BITFUN_E2E_USER_ROOT: userRoot,
    BITFUN_E2E_HOME: userRoot,
    APPDATA: configRoot,
    XDG_CONFIG_HOME: configRoot,
    HOME: userRoot,
    USERPROFILE: userRoot,
  });
  assert.match(result.stdout, /local-package-consumer: PASS/);
  process.stdout.write("local-package-consumer-smoke: PASS\n");
} finally {
  await rm(isolatedRoot, {
    recursive: true,
    force: true,
    maxRetries: 20,
    retryDelay: 100,
  });
}

function runNpm(args, cwd) {
  if (process.platform !== "win32") {
    return run("npm", args, cwd);
  }
  const npmCli = join(
    dirname(process.execPath),
    "node_modules",
    "npm",
    "bin",
    "npm-cli.js",
  );
  return run(process.execPath, [npmCli, ...args], cwd);
}

async function run(command, args, cwd, env = process.env) {
  const child = spawn(command, args, {
    cwd,
    env,
    shell: false,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    stdout = appendBounded(stdout, chunk);
  });
  child.stderr.on("data", (chunk) => {
    stderr = appendBounded(stderr, chunk);
  });
  const exit = await new Promise((resolveExit, rejectExit) => {
    child.once("error", rejectExit);
    child.once("exit", (code, signal) => resolveExit({ code, signal }));
  });
  if (exit.code !== 0) {
    throw new Error(
      `${command} failed with ${exit.signal ?? `exit ${String(exit.code)}`}: ${stderr}`,
    );
  }
  return { stdout, stderr };
}

function appendBounded(current, chunk) {
  const next = current + chunk;
  if (Buffer.byteLength(next, "utf8") > 1024 * 1024) {
    throw new Error("local package command output exceeded its size limit");
  }
  return next;
}
