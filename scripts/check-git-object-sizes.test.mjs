import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';
import test from 'node:test';
import { inspectGitObjectSizes } from './check-git-object-sizes.mjs';

const policy = { maxBlobBytes: 64, allowedBlobs: [] };

function repositoryFixture(t) {
  const temporaryRoot = tmpdir();
  const repository = mkdtempSync(join(temporaryRoot, 'openbitfun-git-object-size-'));
  t.after(() => {
    const relativePath = relative(temporaryRoot, repository);
    assert.match(relativePath, /^openbitfun-git-object-size-[^\\/]+$/);
    rmSync(repository, { recursive: true, force: true });
  });
  const git = (...args) => execFileSync('git', ['-c', 'commit.gpgsign=false', ...args], {
    cwd: repository, encoding: 'utf8', windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'],
  }).trim();
  git('init', '--quiet');
  git('config', 'user.name', 'Object Size Test');
  git('config', 'user.email', 'object-size-test@example.invalid');
  writeFileSync(join(repository, 'source.txt'), 'initial source\n');
  git('add', 'source.txt');
  git('commit', '--quiet', '-m', 'Initial source');
  const base = git('rev-parse', 'HEAD');
  function commitFile(name, content) {
    writeFileSync(join(repository, name), content);
    git('add', '--', name);
    git('commit', '--quiet', '-m', `Update ${name}`);
    return git('rev-parse', `HEAD:${name}`);
  }
  return { repository, base, git, commitFile };
}

test('rejects a large blob added and then deleted within the proposed history', (t) => {
  const fixture = repositoryFixture(t);
  const oid = fixture.commitFile('temporary binary.bin', Buffer.alloc(65, 1));
  fixture.git('rm', '--', 'temporary binary.bin');
  fixture.git('commit', '--quiet', '-m', 'Remove temporary binary');
  const result = inspectGitObjectSizes({ ...fixture, policy });
  assert.deepEqual(result.violations, [{ oid, bytes: 65, path: 'temporary binary.bin' }]);
});

test('does not charge an unchanged object already reachable from the base', (t) => {
  const fixture = repositoryFixture(t);
  fixture.commitFile('existing.bin', Buffer.alloc(65, 2));
  const base = fixture.git('rev-parse', 'HEAD');
  fixture.commitFile('source.txt', 'updated source\n');
  const result = inspectGitObjectSizes({ ...fixture, base, policy });
  assert.equal(result.violations.length, 0);
  assert.equal(result.checkedBlobs, 1);
});

test('allows an exact approved object but rejects replacement bytes at the same path', (t) => {
  const fixture = repositoryFixture(t);
  const oid = fixture.commitFile('font.ttf', Buffer.alloc(65, 3));
  const fontPolicy = { ...policy, allowedBlobs: [{ oid }] };
  assert.equal(inspectGitObjectSizes({ ...fixture, policy: fontPolicy }).violations.length, 0);
  const replacement = fixture.commitFile('font.ttf', Buffer.alloc(65, 4));
  assert.deepEqual(inspectGitObjectSizes({ ...fixture, policy: fontPolicy }).violations.map((entry) => entry.oid), [replacement]);
});

test('full-history checks include existing large objects and invalid bases fail', (t) => {
  const fixture = repositoryFixture(t);
  fixture.commitFile('existing.bin', Buffer.alloc(65, 5));
  const result = inspectGitObjectSizes({ repository: fixture.repository, policy });
  assert.equal(result.violations.length, 1);
  assert.throws(() => inspectGitObjectSizes({ ...fixture, base: 'missing-base', policy }));
});
