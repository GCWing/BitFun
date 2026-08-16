import { spawn } from "node:child_process";

import { SdkError } from "../errors.js";
import type { HostTransport } from "./transport.js";

export interface ManagedHostOptions {
  executable: string;
  args?: readonly string[];
  cwd: string;
  gracefulShutdownMs?: number;
  forceKillTimeoutMs?: number;
}

export interface ManagedHostTransport extends HostTransport {
  exitCode(): number | null;
  hasExited(): boolean;
}

export async function startManagedHost(
  options: ManagedHostOptions,
): Promise<ManagedHostTransport> {
  const child = spawn(options.executable, [...(options.args ?? [])], {
    cwd: options.cwd,
    env: process.env,
    shell: false,
    detached: process.platform !== "win32",
    stdio: ["pipe", "pipe", "pipe"],
    windowsHide: true,
  });

  child.stderr.setEncoding("utf8");
  child.stderr.on("data", () => {
    // Drain diagnostics so a verbose Host cannot block on a full stderr pipe.
  });

  try {
    await new Promise<void>((resolve, reject) => {
      const onSpawn = (): void => {
        child.off("error", onError);
        resolve();
      };
      const onError = (error: Error): void => {
        child.off("spawn", onSpawn);
        reject(error);
      };
      child.once("spawn", onSpawn);
      child.once("error", onError);
    });
  } catch (cause) {
    throw hostStartError(options.executable, cause);
  }

  child.on("error", (error) => {
    child.stdout.destroy(error);
  });

  let observedExitCode: number | null = null;
  let exitObserved = false;
  const exited = new Promise<void>((resolve) => {
    child.once("exit", (code) => {
      observedExitCode = code;
      exitObserved = true;
      resolve();
    });
  });
  let closePromise: Promise<void> | undefined;

  return {
    readable: child.stdout,
    writable: child.stdin,
    close: () => {
      closePromise ??= closeChild();
      return closePromise;
    },
    exitCode: () => observedExitCode ?? child.exitCode,
    hasExited: () => exitObserved,
  };

  async function closeChild(): Promise<void> {
    child.stdin.end();
    const gracefulExit = await settlesWithin(
      exited,
      options.gracefulShutdownMs ?? 5_000,
    );
    const forceKillTimeoutMs = options.forceKillTimeoutMs ?? 2_000;
    if (process.platform !== "win32") {
      await forceKillTree(child.pid, forceKillTimeoutMs);
      if (!gracefulExit && !(await settlesWithin(exited, forceKillTimeoutMs))) {
        throw processTreeCleanupError("direct Host exit was not observed");
      }
      return;
    }
    if (gracefulExit) {
      return;
    }
    await forceKillTree(child.pid, forceKillTimeoutMs);
    if (!(await settlesWithin(exited, forceKillTimeoutMs))) {
      throw new SdkError("SDK Host process tree could not be reclaimed", {
        code: "cleanup_required",
        stage: "shutdown",
        retryable: false,
        correlationId: "local:host_process_tree",
        outcomeCertainty: "unknown",
        recovery: "restart_host",
      });
    }
  }
}

/** @internal */
export async function forceKillTree(
  pid: number | undefined,
  timeoutMs: number,
): Promise<void> {
  if (pid === undefined) {
    throw new SdkError("SDK Host process id is unavailable during cleanup", {
      code: "cleanup_required",
      stage: "shutdown",
      retryable: false,
      correlationId: "local:host_process_tree",
      outcomeCertainty: "unknown",
      recovery: "restart_host",
    });
  }
  if (process.platform !== "win32") {
    try {
      process.kill(-pid, "SIGKILL");
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ESRCH") {
        throw error;
      }
    }
    if (!(await processGroupExited(pid, timeoutMs))) {
      throw processTreeCleanupError("Unix process group remained alive after SIGKILL");
    }
    return;
  }

  const taskkill = spawn("taskkill.exe", ["/pid", String(pid), "/t", "/f"], {
    shell: false,
    stdio: "ignore",
    windowsHide: true,
  });
  const completed = new Promise<void>((resolve, reject) => {
    taskkill.once("error", reject);
    taskkill.once("exit", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(processTreeCleanupError(`taskkill exited with status ${String(code)}`));
      }
    });
  });
  if (!(await settlesWithin(completed, timeoutMs))) {
    taskkill.kill();
    throw new SdkError("Timed out while terminating the SDK Host process tree", {
      code: "cleanup_required",
      stage: "shutdown",
      retryable: false,
      correlationId: "local:host_process_tree",
      outcomeCertainty: "unknown",
      recovery: "restart_host",
    });
  }
}

async function processGroupExited(pid: number, timeoutMs: number): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  do {
    try {
      process.kill(-pid, 0);
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

function processTreeCleanupError(message: string): SdkError {
  return new SdkError(`SDK Host process tree could not be reclaimed: ${message}`, {
    code: "cleanup_required",
    stage: "shutdown",
    retryable: false,
    correlationId: "local:host_process_tree",
    outcomeCertainty: "unknown",
    recovery: "restart_host",
  });
}

async function settlesWithin(
  promise: Promise<void>,
  timeoutMs: number,
): Promise<boolean> {
  let timeout: ReturnType<typeof setTimeout> | undefined;
  const timedOut = new Promise<false>((resolve) => {
    timeout = setTimeout(() => resolve(false), timeoutMs);
  });
  const settled = promise.then(() => true as const);
  const result = await Promise.race([settled, timedOut]);
  if (timeout !== undefined) {
    clearTimeout(timeout);
  }
  return result;
}

function hostStartError(executable: string, cause: unknown): SdkError {
  return new SdkError(
    `SDK Host executable was not found: ${executable}`,
    {
      code: "not_found",
      stage: "initialize",
      retryable: false,
      correlationId: "local:host_start",
      outcomeCertainty: "not_started",
    },
    { cause },
  );
}
