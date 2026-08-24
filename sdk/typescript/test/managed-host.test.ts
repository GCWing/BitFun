import assert from "node:assert/strict";
import { once } from "node:events";
import { basename, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { AgentClient, SdkError } from "../src/index.js";
import { createAgentClient } from "../src/internal/client.js";
import { resolveHostPath } from "../src/internal/host-path.js";
import { forceKillTree, startManagedHost } from "../src/internal/managed-host.js";
import type { AgentClientOptions } from "../src/types.js";

const model = {
  provider: "openai" as const,
  model: "fixture-model",
  apiKey: "fixture-secret",
  baseUrl: "http://127.0.0.1:43123/v1",
};

test("the managed transport owns one child Host process", async () => {
  const fixture = fileURLToPath(
    new URL("../../../../test/fixtures/host.mjs", import.meta.url),
  );
  const transport = await startManagedHost({
    executable: process.execPath,
    args: [fixture],
    cwd: process.cwd(),
  });
  const client = await createAgentClient(transport, { cwd: process.cwd(), model });

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
      model,
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

test("the Host resolver uses the package-local executable without environment fallback", () => {
  const previousHostPath = process.env.BITFUN_SDK_HOST_PATH;
  process.env.BITFUN_SDK_HOST_PATH = process.execPath;
  try {
    const executableName = process.platform === "win32" ? "bitfun-sdk-host.exe" : "bitfun-sdk-host";
    const expectedHost = fileURLToPath(
      new URL(
        `../native/${process.platform}-${process.arch}/${executableName}`,
        import.meta.url,
      ),
    );
    assert.equal(resolveHostPath(), expectedHost);
    assert.notEqual(resolveHostPath(), process.execPath);
  } finally {
    if (previousHostPath === undefined) {
      delete process.env.BITFUN_SDK_HOST_PATH;
    } else {
      process.env.BITFUN_SDK_HOST_PATH = previousHostPath;
    }
  }
});

test("AgentClient.start rejects a relative Host override before spawning", async () => {
  await assert.rejects(
    AgentClient.start({
      cwd: dirname(process.execPath),
      hostPath: basename(process.execPath),
      initializeTimeoutMs: 100,
      model,
    }),
    (error: unknown) => {
      assert.ok(error instanceof SdkError);
      assert.equal(error.code, "invalid_request");
      assert.equal(error.stage, "initialize");
      assert.equal(error.outcomeCertainty, "not_started");
      assert.doesNotMatch(String(error.stack), /fixture-secret/);
      return true;
    },
  );
});

test("AgentClient.start rejects invalid model options before spawning a Host", async (context) => {
  const missingHost = fileURLToPath(
    new URL("../../../../test/fixtures/missing-host", import.meta.url),
  );
  const cases: Array<{ name: string; options: unknown }> = [
    {
      name: "missing model",
      options: { cwd: process.cwd(), hostPath: missingHost },
    },
    {
      name: "blank model",
      options: { cwd: process.cwd(), hostPath: missingHost, model: { ...model, model: " " } },
    },
    {
      name: "blank API key",
      options: { cwd: process.cwd(), hostPath: missingHost, model: { ...model, apiKey: " " } },
    },
    {
      name: "unsupported provider",
      options: {
        cwd: process.cwd(),
        hostPath: missingHost,
        model: { ...model, provider: "unsupported" },
      },
    },
    {
      name: "invalid base URL",
      options: {
        cwd: process.cwd(),
        hostPath: missingHost,
        model: { ...model, baseUrl: "not an absolute URL" },
      },
    },
  ];

  for (const fixture of cases) {
    await context.test(fixture.name, async () => {
      await assert.rejects(
        AgentClient.start(fixture.options as AgentClientOptions),
        (error: unknown) => {
          assert.ok(error instanceof SdkError);
          assert.equal(error.code, "invalid_request");
          assert.equal(error.stage, "initialize");
          assert.equal(error.retryable, false);
          assert.equal(error.outcomeCertainty, "not_started");
          assert.doesNotMatch(String(error.stack), /fixture-secret/);
          return true;
        },
      );
    });
  }
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
