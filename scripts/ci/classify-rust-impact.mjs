import { appendFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { rustWebUiSourceBoundaryRule } from '../core-boundaries/rules/source-rules.mjs';
import { scanForbiddenContentUnder } from '../core-boundaries/source-content-checks.mjs';

function readArg(args, name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
}

function changedPaths(base, head, rangeMode) {
  const rangeArgs = rangeMode === 'merge-base'
    ? [`${base}...${head}`]
    : [base, head];
  const result = spawnSync(
    'git',
    ['diff', '--no-renames', '--name-only', '-z', ...rangeArgs, '--'],
    { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
  );
  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || 'git diff failed');
  }
  return result.stdout.split('\0').filter(Boolean);
}

export function classifyRustImpact(paths) {
  const result = (rustRequired, reason) => ({
    rustRequired,
    reason,
    changedCount: paths.length,
  });
  if (paths.length === 0) {
    return result(true, 'no-changes');
  }
  if (paths.some((file) => !isValidRepositoryPath(file))) {
    return result(true, 'invalid-path');
  }
  if (paths.some(isRustBuildInput)) {
    return result(true, 'rust-build-input');
  }

  const activePaths = paths.filter((file) => !isKnownNeutralPath(file));
  if (activePaths.length === 0) {
    return result(false, 'ci-ignored-only');
  }
  if (activePaths.every((file) => file.startsWith('src/web-ui/'))) {
    return result(false, 'web-ui-only');
  }
  return result(true, 'outside-web-ui');
}

function isValidRepositoryPath(file) {
  return typeof file === 'string'
    && file.length > 0
    && !file.startsWith('/')
    && !/^[A-Za-z]:/.test(file)
    && !file.includes('\\')
    && !/[\r\n\0]/.test(file)
    && file.split('/').every((segment) => segment !== '' && segment !== '.' && segment !== '..');
}

function isRustBuildInput(file) {
  const segments = file.split('/');
  const name = segments.at(-1);
  return file.endsWith('.rs')
    || name === 'Cargo.toml'
    || name === 'Cargo.lock'
    || name === 'build.rs'
    || name === 'rust-toolchain'
    || name === 'rust-toolchain.toml'
    || segments.includes('.cargo');
}

function isKnownNeutralPath(file) {
  const isKnownDocumentation = file.endsWith('.md')
    && (!file.includes('/') || file.startsWith('docs/'));
  return isKnownDocumentation || file.startsWith('png/');
}

export function run(args = process.argv.slice(2), env = process.env) {
  const base = readArg(args, '--base');
  const head = readArg(args, '--head');
  const rangeMode = readArg(args, '--range-mode') ?? 'direct';
  if (!base || !head) {
    throw new Error(
      'Usage: classify-rust-impact.mjs --base <sha> --head <sha> '
      + '[--range-mode direct|merge-base]',
    );
  }

  const boundaryFindings = scanForbiddenContentUnder(
    process.cwd(),
    rustWebUiSourceBoundaryRule,
  );
  if (boundaryFindings.length > 0) {
    const details = boundaryFindings
      .slice(0, 20)
      .map((finding) => `${finding.repoPath}:${finding.line}: ${finding.message}`)
      .join('\n');
    throw new Error(`${rustWebUiSourceBoundaryRule.reason}\n${details}`);
  }

  let paths = [];
  let result;
  if (
    !isUsableCommitSha(base)
    || !isUsableCommitSha(head)
    || !['direct', 'merge-base'].includes(rangeMode)
  ) {
    result = { rustRequired: true, reason: 'invalid-range', changedCount: 0 };
  } else {
    try {
      paths = changedPaths(base, head, rangeMode);
      result = classifyRustImpact(paths);
    } catch {
      result = { rustRequired: true, reason: 'unavailable-range', changedCount: 0 };
    }
  }
  const lines = [
    `rust_required=${result.rustRequired}`,
    `reason=${result.reason}`,
    `changed_count=${result.changedCount}`,
  ];
  if (env.GITHUB_OUTPUT) {
    appendFileSync(env.GITHUB_OUTPUT, `${lines.join('\n')}\n`);
  } else {
    process.stdout.write(`${lines.join('\n')}\n`);
  }
  if (env.GITHUB_STEP_SUMMARY) {
    appendFileSync(env.GITHUB_STEP_SUMMARY, renderSummary(result, paths));
  }
  return result;
}

function isUsableCommitSha(value) {
  return /^[0-9a-f]{40}$/i.test(value) && !/^0{40}$/.test(value);
}

function renderSummary(result, paths) {
  const shownPaths = paths.slice(0, 20);
  const lines = [
    '### Rust/CLI impact classification',
    '',
    `- <strong>Required:</strong> ${result.rustRequired}`,
    `- <strong>Reason:</strong> ${result.reason}`,
    `- <strong>Changed files:</strong> ${result.changedCount}`,
  ];
  if (shownPaths.length > 0) {
    lines.push('', ...shownPaths.map((file) => `- <code>${escapeHtml(file)}</code>`));
  }
  if (paths.length > shownPaths.length) {
    lines.push(`- ${paths.length - shownPaths.length} additional changed file(s) omitted`);
  }
  return `${lines.join('\n')}\n`;
}

function escapeHtml(value) {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('\r', '\\r')
    .replaceAll('\n', '\\n');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    run();
  } catch (error) {
    process.stderr.write(`${error.message || String(error)}\n`);
    process.exitCode = 1;
  }
}
