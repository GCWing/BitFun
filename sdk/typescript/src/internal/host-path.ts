import { isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";

import { SdkError } from "../errors.js";

export function resolveHostPath(explicitPath?: string): string {
  if (explicitPath === undefined) {
    return packageHostPath(process.platform, process.arch);
  }
  if (typeof explicitPath !== "string" || !isAbsolute(explicitPath)) {
    throw new SdkError("SDK Host path must be an explicit absolute path", {
      code: "invalid_request",
      stage: "initialize",
      retryable: false,
      correlationId: "local:host_validation",
      outcomeCertainty: "not_started",
    });
  }
  return explicitPath;
}

export function packageHostPath(
  platform: NodeJS.Platform,
  arch: NodeJS.Architecture,
): string {
  const executable = platform === "win32" ? "bitfun-sdk-host.exe" : "bitfun-sdk-host";
  return fileURLToPath(
    new URL(`../../native/${platform}-${arch}/${executable}`, import.meta.url),
  );
}
