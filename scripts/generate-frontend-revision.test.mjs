import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
  FRONTEND_REVISION_ALGORITHM,
  FRONTEND_REVISION_MANIFEST,
  generateFrontendRevisionManifest,
} from './generate-frontend-revision.mjs';

async function frontendFixture() {
  const root = await mkdtemp(path.join(tmpdir(), 'openbitfun-frontend-revision-'));
  await mkdir(path.join(root, 'assets'));
  await writeFile(path.join(root, 'index.html'), '<main>OpenBitFun</main>');
  await writeFile(path.join(root, 'assets', 'app.js'), 'export const value = 1;');
  return root;
}

test('frontend revision manifest is stable and excludes itself', async () => {
  const root = await frontendFixture();
  try {
    const first = await generateFrontendRevisionManifest(root);
    const second = await generateFrontendRevisionManifest(root);
    const written = JSON.parse(
      await readFile(path.join(root, FRONTEND_REVISION_MANIFEST), 'utf8'),
    );

    assert.deepEqual(second, first);
    assert.deepEqual(written, first);
    assert.equal(first.schemaVersion, 1);
    assert.equal(first.algorithm, FRONTEND_REVISION_ALGORITHM);
    assert.equal(first.fileCount, 2);
    assert.equal(first.digest.length, 64);
    assert.equal(first.revision, `bundled-${first.digest.slice(0, 16)}`);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('frontend revision covers assets beyond index.html', async () => {
  const root = await frontendFixture();
  try {
    const first = await generateFrontendRevisionManifest(root);
    await writeFile(path.join(root, 'assets', 'app.js'), 'export const value = 2;');
    const second = await generateFrontendRevisionManifest(root);

    assert.notEqual(second.revision, first.revision);
    assert.notEqual(second.digest, first.digest);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
