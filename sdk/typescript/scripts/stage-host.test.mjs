import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

test("stageHost copies an already-built Host into its package destination", async () => {
  const stageHost = await loadStageHost();
  const root = await mkdtemp(join(tmpdir(), "openbitfun-sdk-stage-host-"));
  const source = join(root, "source-host");
  const destination = join(root, "package", "native", "host");
  const contents = Buffer.from("local-host-fixture\n", "utf8");
  try {
    await writeFile(source, contents, { mode: 0o600 });

    await stageHost(source, destination);

    assert.deepEqual(await readFile(destination), contents);
    if (process.platform !== "win32") {
      assert.notEqual((await stat(destination)).mode & 0o111, 0);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("stageHost rejects a directory source", async () => {
  const stageHost = await loadStageHost();
  const root = await mkdtemp(join(tmpdir(), "openbitfun-sdk-stage-host-invalid-"));
  try {
    const source = join(root, "source-directory");
    await mkdir(source);
    await assert.rejects(
      stageHost(source, join(root, "destination")),
      /Host source must be a file/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

async function loadStageHost() {
  try {
    const module = await import("./stage-host.mjs");
    assert.equal(typeof module.stageHost, "function");
    return module.stageHost;
  } catch (error) {
    if (error?.code === "ERR_MODULE_NOT_FOUND") {
      assert.fail("stage-host.mjs must export stageHost");
    }
    throw error;
  }
}
