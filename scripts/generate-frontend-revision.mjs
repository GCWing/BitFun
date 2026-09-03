#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { lstat, readFile, readdir, rename, rm, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const FRONTEND_REVISION_MANIFEST = 'frontend-revision.json';
export const FRONTEND_REVISION_ALGORITHM = 'sha256-path-content-v1';

const ROOT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const DEFAULT_DIST_DIR = path.join(ROOT_DIR, 'dist');

function uint64LittleEndian(value) {
  const bytes = Buffer.allocUnsafe(8);
  bytes.writeBigUInt64LE(BigInt(value));
  return bytes;
}

async function collectFiles(root, directory = root, files = []) {
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((left, right) => (left.name < right.name ? -1 : left.name > right.name ? 1 : 0));

  for (const entry of entries) {
    if (
      directory === root &&
      (entry.name === FRONTEND_REVISION_MANIFEST ||
        (entry.name.startsWith(`${FRONTEND_REVISION_MANIFEST}.`) && entry.name.endsWith('.tmp')))
    ) {
      continue;
    }
    const absolutePath = path.join(directory, entry.name);
    const metadata = await lstat(absolutePath);
    if (metadata.isSymbolicLink()) {
      throw new Error(`Frontend bundles cannot contain symbolic links: ${absolutePath}`);
    }
    if (metadata.isDirectory()) {
      await collectFiles(root, absolutePath, files);
    } else if (metadata.isFile()) {
      files.push({
        absolutePath,
        relativePath: path.relative(root, absolutePath).split(path.sep).join('/'),
      });
    }
  }
  return files;
}

export async function generateFrontendRevisionManifest(distDir = DEFAULT_DIST_DIR) {
  const indexPath = path.join(distDir, 'index.html');
  const indexMetadata = await stat(indexPath).catch(() => null);
  if (!indexMetadata?.isFile()) {
    throw new Error(`Frontend build output has no index.html: ${distDir}`);
  }

  const files = await collectFiles(distDir);
  files.sort((left, right) =>
    left.relativePath < right.relativePath ? -1 : left.relativePath > right.relativePath ? 1 : 0,
  );
  const hasher = createHash('sha256');
  let totalBytes = 0;

  for (const file of files) {
    const relativePath = Buffer.from(file.relativePath, 'utf8');
    const contents = await readFile(file.absolutePath);
    hasher.update(uint64LittleEndian(relativePath.byteLength));
    hasher.update(relativePath);
    hasher.update(uint64LittleEndian(contents.byteLength));
    hasher.update(contents);
    totalBytes += contents.byteLength;
  }

  const digest = hasher.digest('hex');
  const manifest = {
    schemaVersion: 1,
    revision: `bundled-${digest.slice(0, 16)}`,
    algorithm: FRONTEND_REVISION_ALGORITHM,
    digest,
    fileCount: files.length,
    totalBytes,
  };
  const destination = path.join(distDir, FRONTEND_REVISION_MANIFEST);
  const temporary = `${destination}.${process.pid}.tmp`;
  await writeFile(temporary, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
  await rm(destination, { force: true });
  await rename(temporary, destination);
  return manifest;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const distDir = process.argv[2] ? path.resolve(process.argv[2]) : DEFAULT_DIST_DIR;
  generateFrontendRevisionManifest(distDir)
    .then((manifest) => {
      process.stdout.write(
        `[frontend-revision] generated ${manifest.revision} (${manifest.fileCount} files, ${manifest.totalBytes} bytes)\n`,
      );
    })
    .catch((error) => {
      process.stderr.write(`[frontend-revision] ${error.message}\n`);
      process.exitCode = 1;
    });
}
