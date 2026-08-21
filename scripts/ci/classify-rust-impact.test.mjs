import assert from 'node:assert/strict';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { classifyRustImpact } from './classify-rust-impact.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const scriptPath = path.join(repoRoot, 'scripts/ci/classify-rust-impact.mjs');

function git(root, args) {
  const result = spawnSync('git', args, {
    cwd: root,
    encoding: 'utf8',
  });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim();
}

function commit(root, message) {
  git(root, ['add', '--all']);
  git(root, [
    '-c',
    'user.name=CI Impact Test',
    '-c',
    'user.email=ci-impact@example.invalid',
    'commit',
    '-m',
    message,
  ]);
  return git(root, ['rev-parse', 'HEAD']);
}

function parseOutputs(file) {
  return Object.fromEntries(
    readFileSync(file, 'utf8')
      .trim()
      .split(/\r?\n/)
      .map((line) => line.split('=', 2)),
  );
}

function runClassifier(root, base, head, rangeMode = 'direct') {
  const output = path.join(root, `github-output-${Date.now()}-${Math.random()}.txt`);
  const summary = path.join(root, `github-summary-${Date.now()}-${Math.random()}.md`);
  const result = spawnSync(
    process.execPath,
    [scriptPath, '--base', base, '--head', head, '--range-mode', rangeMode],
    {
      cwd: root,
      env: {
        ...process.env,
        GITHUB_OUTPUT: output,
        GITHUB_STEP_SUMMARY: summary,
      },
      encoding: 'utf8',
    },
  );
  return {
    ...result,
    outputs: result.status === 0 ? parseOutputs(output) : undefined,
    summary: existsSync(summary) ? readFileSync(summary, 'utf8') : undefined,
  };
}

test('skips Rust validation when a commit changes only Web UI files', (t) => {
  const root = mkdtempSync(path.join(tmpdir(), 'bitfun-rust-impact-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));

  git(root, ['init', '--initial-branch=main']);
  writeFileSync(path.join(root, 'README.md'), 'baseline\n');
  const base = commit(root, 'baseline');

  const webFile = path.join(root, 'src/web-ui/src/example.ts');
  mkdirSync(path.dirname(webFile), { recursive: true });
  writeFileSync(webFile, 'export const example = true;\n');
  const head = commit(root, 'web change');

  const result = runClassifier(root, base, head);

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.outputs, {
    rust_required: 'false',
    reason: 'web-ui-only',
    changed_count: '1',
  });
  assert.match(result.summary, /Rust\/CLI impact classification/);
  assert.match(result.summary, /Required:<\/strong> false/);
  assert.match(result.summary, /Reason:<\/strong> web-ui-only/);
  assert.match(result.summary, /<code>src\/web-ui\/src\/example\.ts<\/code>/);
});

test('uses a merge-base range for pull requests but a direct range for pushes', (t) => {
  const root = mkdtempSync(path.join(tmpdir(), 'bitfun-rust-impact-diverged-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));

  git(root, ['init', '--initial-branch=main']);
  writeFileSync(path.join(root, 'README.md'), 'baseline\n');
  commit(root, 'baseline');

  git(root, ['switch', '-c', 'feature']);
  const webFile = path.join(root, 'src/web-ui/src/example.ts');
  mkdirSync(path.dirname(webFile), { recursive: true });
  writeFileSync(webFile, 'export const example = true;\n');
  const head = commit(root, 'web change');

  git(root, ['switch', 'main']);
  const rustFile = path.join(root, 'src/apps/desktop/src/lib.rs');
  mkdirSync(path.dirname(rustFile), { recursive: true });
  writeFileSync(rustFile, 'pub fn base_change() {}\n');
  const currentBase = commit(root, 'base Rust change');

  assert.deepEqual(runClassifier(root, currentBase, head, 'merge-base').outputs, {
    rust_required: 'false',
    reason: 'web-ui-only',
    changed_count: '1',
  });
  assert.deepEqual(runClassifier(root, currentBase, head, 'direct').outputs, {
    rust_required: 'true',
    reason: 'rust-build-input',
    changed_count: '2',
  });
});

test('keeps workflow-ignored documentation neutral beside Web UI changes', () => {
  assert.deepEqual(
    classifyRustImpact([
      'src/web-ui/src/example.ts',
      'src/web-ui/README.md',
      'docs/review-notes.md',
      'png/example/screenshot.png',
    ]),
    {
      rustRequired: false,
      reason: 'web-ui-only',
      changedCount: 4,
    },
  );
  assert.deepEqual(classifyRustImpact(['docs/review-notes.md', 'png/example.png']), {
    rustRequired: false,
    reason: 'ci-ignored-only',
    changedCount: 2,
  });
  assert.deepEqual(
    classifyRustImpact([
      'src/web-ui/src/example.ts',
      'src/crates/assembly/agent-content/prompts/agents/example.md',
    ]),
    {
      rustRequired: true,
      reason: 'outside-web-ui',
      changedCount: 2,
    },
    'nested Markdown may be a Rust include input and must fail closed',
  );
});

test('requires Rust for ambiguous, native, or cross-boundary path sets', () => {
  for (const [paths, reason] of [
    [[], 'no-changes'],
    [['src/apps/desktop/src/lib.rs'], 'rust-build-input'],
    [['src/web-ui/native/build.rs'], 'rust-build-input'],
    [['src/web-ui/native/Cargo.toml'], 'rust-build-input'],
    [['src/web-ui/src/example.ts', 'scripts/check-core-boundaries.mjs'], 'outside-web-ui'],
    [['src/web-ui/../apps/desktop/src/lib.rs'], 'invalid-path'],
    [['src\\web-ui\\src\\example.ts'], 'invalid-path'],
  ]) {
    assert.deepEqual(classifyRustImpact(paths), {
      rustRequired: true,
      reason,
      changedCount: paths.length,
    });
  }
});

test('requires Rust when the event range is invalid or unavailable', (t) => {
  const root = mkdtempSync(path.join(tmpdir(), 'bitfun-rust-impact-range-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));

  git(root, ['init', '--initial-branch=main']);
  writeFileSync(path.join(root, 'README.md'), 'baseline\n');
  const head = commit(root, 'baseline');

  for (const [base, reason] of [
    ['0'.repeat(40), 'invalid-range'],
    ['f'.repeat(40), 'unavailable-range'],
  ]) {
    const result = runClassifier(root, base, head);
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(result.outputs, {
      rust_required: 'true',
      reason,
      changed_count: '0',
    });
  }

  const invalidMode = runClassifier(root, head, head, 'unsupported');
  assert.equal(invalidMode.status, 0, invalidMode.stderr);
  assert.deepEqual(invalidMode.outputs, {
    rust_required: 'true',
    reason: 'invalid-range',
    changed_count: '0',
  });
});

test('rejects tracked Rust sources that spell a Web UI input token', (t) => {
  const root = mkdtempSync(path.join(tmpdir(), 'bitfun-rust-impact-boundary-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));

  git(root, ['init', '--initial-branch=main']);
  writeFileSync(path.join(root, 'README.md'), 'baseline\n');
  const base = commit(root, 'baseline');

  const rustFile = path.join(root, 'src/lib.rs');
  mkdirSync(path.dirname(rustFile), { recursive: true });
  writeFileSync(rustFile, 'const WEB: &str = include_dir!("../web-ui/src");\n');
  const head = commit(root, 'forbidden Rust input');

  const result = runClassifier(root, base, head);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Rust source must not reference the Web UI source tree/);
});
