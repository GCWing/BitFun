import assert from "node:assert/strict";
import { once } from "node:events";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { AgentClient, SdkError } from "../src/index.js";
import { createAgentClient } from "../src/internal/client.js";
import { forceKillTree, startManagedHost } from "../src/internal/managed-host.js";

test("the managed transport owns one child Host process", async () => {
  const fixture = fileURLToPath(
    new URL("../../../../test/fixtures/host.mjs", import.meta.url),
  );
  const transport = await startManagedHost({
    executable: process.execPath,
    args: [fixture],
    cwd: process.cwd(),
  });
  const client = await createAgentClient(transport, { cwd: process.cwd() });

  assert.ok(client instanceof AgentClient);
  await client.close();
  assert.equal(transport.exitCode(), 0);
});

test("AgentClient.start reports a missing Host before an operation begins", async () => {
  await assert.rejects(
    AgentClient.start({
      cwd: process.cwd(),
      hostPath: fileURLToPath(
        new URL("../../../../test/fixtures/missing-host", import.meta.url),
      ),
    }),
    (error: unknown) => {
      assert.ok(error instanceof SdkError);
      assert.equal(error.code, "not_found");
      assert.equal(error.stage, "initialize");
      assert.equal(error.outcomeCertainty, "not_started");
      return true;
    },
  );
});

test("forced managed Host cleanup reclaims its descendant process tree", async () => {
  const fixture = fileURLToPath(
    new URL("../../../../test/fixtures/unresponsive-host.mjs", import.meta.url),
  );
  const options = {
    executable: process.execPath,
    args: [fixture],
    cwd: process.cwd(),
    gracefulShutdownMs: 20,
    forceKillTimeoutMs: 2_000,
  } as Parameters<typeof startManagedHost>[0];
  const transport = await startManagedHost(options);
  const [chunk] = (await once(transport.readable, "data")) as [Buffer];
  const descendantPid = Number.parseInt(chunk.toString("utf8").trim(), 10);
  assert.equal(Number.isSafeInteger(descendantPid), true);

  try {
    await transport.close();
    assert.equal(transport.hasExited(), true);
    assert.equal(await processExited(descendantPid), true);
  } finally {
    if (!(await processExited(descendantPid, 50))) {
      try {
        process.kill(descendantPid);
      } catch {
        // The descendant exited between the final probe and cleanup.
      }
    }
  }
});

test(
  "Unix cleanup reclaims the process group after the direct Host exits first",
  { skip: process.platform === "win32" },
  async () => {
    const fixture = fileURLToPath(
      new URL("../../../../test/fixtures/unresponsive-host.mjs", import.meta.url),
    );
    const transport = await startManagedHost({
      executable: process.execPath,
      args: [fixture, "exit-parent"],
      cwd: process.cwd(),
      gracefulShutdownMs: 20,
      forceKillTimeoutMs: 2_000,
    });
    const [chunk] = (await once(transport.readable, "data")) as [Buffer];
    const descendantPid = Number.parseInt(chunk.toString("utf8").trim(), 10);
    if (!transport.readable.readableEnded) {
      await once(transport.readable, "end");
    }
    assert.equal(transport.hasExited(), true);

    try {
      await transport.close();
      assert.equal(await processExited(descendantPid), true);
    } finally {
      if (!(await processExited(descendantPid, 50))) {
        try {
          process.kill(descendantPid);
        } catch {
          // The descendant exited between the final probe and cleanup.
        }
      }
    }
  },
);

test(
  "Windows process-tree cleanup rejects a non-zero taskkill result",
  { skip: process.platform !== "win32" },
  async () => {
    await assert.rejects(forceKillTree(0, 2_000), (error: unknown) => {
      assert.ok(error instanceof SdkError);
      assert.equal(error.code, "cleanup_required");
      assert.equal(error.outcomeCertainty, "unknown");
      return true;
    });
  },
);

async function processExited(pid: number, timeoutMs = 1_000): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  do {
    try {
      process.kill(pid, 0);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ESRCH") {
        return true;
      }
      throw error;
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  } while (Date.now() < deadline);
  return false;
}
