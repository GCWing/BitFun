import { chmod, copyFile, mkdir, stat } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

export async function stageHost(source, destination) {
  let sourceMetadata;
  try {
    sourceMetadata = await stat(source);
  } catch (cause) {
    throw new Error(`Host source was not found: ${source}`, { cause });
  }
  if (!sourceMetadata.isFile()) {
    throw new Error(`Host source must be a file: ${source}`);
  }

  await mkdir(dirname(destination), { recursive: true });
  await copyFile(source, destination);
  if (process.platform !== "win32") {
    await chmod(destination, 0o755);
  }
}

async function main() {
  const [source, ...extra] = process.argv.slice(2);
  if (source === undefined || extra.length > 0) {
    throw new Error("Usage: pnpm stage:host -- <host-executable>");
  }

  const { packageHostPath } = await import(
    "../dist/sdk/typescript/src/internal/host-path.js"
  );
  const destination = packageHostPath(process.platform, process.arch);
  await stageHost(resolve(source), destination);
  process.stdout.write(`Staged OpenBitFun SDK Host at ${destination}\n`);
}

if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  try {
    await main();
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
