import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(SCRIPT_DIR, '..', '..', '..');
const REPORT_DIR = path.join(ROOT, 'tests', 'e2e', 'reports', 'performance');
const PERF_RUN_ROOT = path.join(ROOT, 'tests', 'e2e', '.bitfun', 'perf-runs');
const FIXTURE_SCRIPT = path.join(ROOT, 'tests', 'e2e', 'scripts', 'generate-long-session-fixture.mjs');
const DEFAULT_LONG_SESSION_TARGET_ID = 'perf-long-session-001';
const DEFAULT_RAPID_SWITCH_SESSION_IDS = [
  'perf-rapid-a-000',
  'perf-rapid-b-000',
  'perf-rapid-c-000',
];
const MAX_RETAINED_PERF_RUNS = 8;
const matrixRunId = `${new Date().toISOString().replace(/[:.]/g, '-')}-${process.pid}`;

const scenarios = {
  'first-open': {
    spec: './specs/performance/startup-session-perf.spec.ts',
    grep: 'collects first-open timing for a generated long session',
    reportPrefix: 'long-session-first-open-',
  },
  'warm-reopen': {
    spec: './specs/performance/startup-session-perf.spec.ts',
    grep: 'collects warm-reopen timing for a generated long session',
    reportPrefix: 'long-session-warm-reopen-',
  },
  'rapid-switch-zero-delay': {
    spec: './specs/performance/startup-session-perf.spec.ts',
    grep: 'collects rapid-switch timing across generated long sessions',
    reportPrefix: 'long-session-rapid-switch-',
    env: {
      BITFUN_E2E_PERF_RAPID_SWITCH_DELAY_MS: '0',
    },
  },
  'first-scroll': {
    spec: './specs/performance/startup-session-perf.spec.ts',
    grep: 'collects first-open timing for a generated long session',
    reportPrefix: 'long-session-first-open-',
    env: {
      BITFUN_E2E_PERF_POST_VISIBLE_INTERACTION: 'first-scroll',
    },
  },
  'scroll-down': {
    spec: './specs/performance/startup-session-perf.spec.ts',
    grep: 'collects first-open timing for a generated long session',
    reportPrefix: 'long-session-first-open-',
    env: {
      BITFUN_E2E_PERF_POST_VISIBLE_INTERACTION: 'scroll-down',
    },
  },
  'turn-navigation': {
    spec: './specs/l1-chat-turn-navigation-release.spec.ts',
    reportPrefix: null,
    env: {
      BITFUN_E2E_TURN_NAV_SESSION_ID: DEFAULT_LONG_SESSION_TARGET_ID,
      BITFUN_E2E_TURN_NAV_TARGET_INDEX: '20',
    },
  },
  'resize-window': {
    spec: './specs/performance/startup-session-perf.spec.ts',
    grep: 'collects first-open timing for a generated long session',
    reportPrefix: 'long-session-first-open-',
    env: {
      BITFUN_E2E_PERF_POST_VISIBLE_INTERACTION: 'resize-window',
    },
  },
  'resize-window-width': {
    spec: './specs/performance/startup-session-perf.spec.ts',
    grep: 'collects first-open timing for a generated long session',
    reportPrefix: 'long-session-first-open-',
    env: {
      BITFUN_E2E_PERF_POST_VISIBLE_INTERACTION: 'resize-window-width',
    },
  },
  'input-layout': {
    spec: './specs/performance/session-input-layout.spec.ts',
    reportPrefix: null,
  },
};

const profiles = {
  core: ['first-open', 'rapid-switch-zero-delay', 'first-scroll', 'turn-navigation', 'resize-window-width'],
  scroll: ['first-scroll', 'scroll-down', 'turn-navigation'],
  resize: ['resize-window', 'resize-window-width'],
  full: [
    'first-open',
    'warm-reopen',
    'rapid-switch-zero-delay',
    'first-scroll',
    'scroll-down',
    'turn-navigation',
    'resize-window',
    'resize-window-width',
    'input-layout',
  ],
};

function shellQuote(value) {
  if (process.platform === 'win32') {
    return `"${String(value).replace(/"/g, '\\"')}"`;
  }
  return `'${String(value).replace(/'/g, "'\\''")}'`;
}

function runPnpm(args, options) {
  return spawnSync(['pnpm', ...args.map(shellQuote)].join(' '), {
    ...options,
    shell: true,
  });
}

function runNode(args, options) {
  return spawnSync(process.execPath, args, {
    ...options,
    shell: false,
    encoding: 'utf8',
  });
}

function runnerStdioOptions() {
  if (process.env.BITFUN_E2E_PERF_RUNNER_STREAM_LOGS === '1') {
    return { stdio: 'inherit' };
  }
  return { stdio: 'pipe', encoding: 'utf8' };
}

function outputTail(result) {
  if (!result.stdout && !result.stderr) {
    return '';
  }
  return [result.stdout, result.stderr]
    .filter(Boolean)
    .join('\n')
    .split(/\r?\n/)
    .slice(-80)
    .join('\n');
}

function argValue(name) {
  const index = process.argv.indexOf(name);
  if (index < 0) {
    return undefined;
  }
  return process.argv[index + 1];
}

function hasFlag(name) {
  return process.argv.includes(name);
}

function allowMissingReports() {
  return (
    hasFlag('--allow-missing-reports') ||
    process.env.BITFUN_E2E_PERF_ALLOW_MISSING_REPORTS === '1'
  );
}

function safePathSegment(value) {
  return String(value).replace(/[^a-zA-Z0-9_.-]/g, '-');
}

function assertPathWithin(parent, candidate) {
  const relative = path.relative(parent, candidate);
  if (relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative))) {
    return;
  }
  throw new Error(`Refusing to clean path outside performance run root: ${candidate}`);
}

function pruneOldPerfRuns(maxRuns = MAX_RETAINED_PERF_RUNS) {
  if (!fs.existsSync(PERF_RUN_ROOT)) {
    return;
  }

  const runs = fs
    .readdirSync(PERF_RUN_ROOT, { withFileTypes: true })
    .filter(entry => entry.isDirectory())
    .map(entry => {
      const fullPath = path.join(PERF_RUN_ROOT, entry.name);
      const stat = fs.statSync(fullPath);
      return { fullPath, mtimeMs: stat.mtimeMs };
    })
    .sort((left, right) => right.mtimeMs - left.mtimeMs);

  for (const run of runs.slice(maxRuns)) {
    assertPathWithin(PERF_RUN_ROOT, run.fullPath);
    fs.rmSync(run.fullPath, { recursive: true, force: true });
  }
}

function runFixture(args, env) {
  const result = runNode([FIXTURE_SCRIPT, ...args], {
    cwd: ROOT,
    env,
  });
  if (result.status !== 0) {
    throw new Error(
      `Failed to generate long-session fixture.\n${outputTail(result)}`,
    );
  }
}

function prepareScenarioRuntime(name, baseEnv) {
  const scenarioRoot = path.join(PERF_RUN_ROOT, matrixRunId, safePathSegment(name));
  assertPathWithin(PERF_RUN_ROOT, scenarioRoot);
  fs.rmSync(scenarioRoot, { recursive: true, force: true });

  const storageRoot = path.join(scenarioRoot, 'storage');
  const workspace = path.join(scenarioRoot, 'workspace');
  const homeRoot = path.join(storageRoot, 'home');
  const userRoot = path.join(storageRoot, 'user-root');
  const logRoot = path.join(storageRoot, 'logs');
  fs.mkdirSync(workspace, { recursive: true });
  fs.writeFileSync(
    path.join(workspace, 'README.md'),
    '# BitFun performance fixture workspace\n',
    'utf8',
  );

  const env = {
    ...baseEnv,
    BITFUN_E2E_STORAGE_ROOT: storageRoot,
    BITFUN_E2E_HOME: homeRoot,
    BITFUN_HOME: homeRoot,
    BITFUN_E2E_USER_ROOT: userRoot,
    BITFUN_USER_ROOT: userRoot,
    BITFUN_E2E_LOG_DIR: logRoot,
    E2E_TEST_WORKSPACE: workspace,
    BITFUN_E2E_PERF_SESSION_ID: DEFAULT_LONG_SESSION_TARGET_ID,
    BITFUN_E2E_PERF_RAPID_SWITCH_SESSION_IDS: DEFAULT_RAPID_SWITCH_SESSION_IDS.join(','),
  };
  const timestampBase = Date.now();

  runFixture([
    '--workspace',
    workspace,
    '--bitfun-home',
    homeRoot,
    '--bitfun-user-root',
    userRoot,
    '--session-prefix',
    'perf-long-session',
    '--session-count',
    '80',
    '--long-session-index',
    '1',
    '--last-active-at-base',
    String(timestampBase),
  ], env);

  ['perf-rapid-a', 'perf-rapid-b', 'perf-rapid-c'].forEach((prefix, index) => {
    runFixture([
      '--workspace',
      workspace,
      '--bitfun-home',
      homeRoot,
      '--bitfun-user-root',
      userRoot,
      '--session-prefix',
      prefix,
      '--session-count',
      '1',
      '--long-session-index',
      '0',
      '--last-active-at-base',
      String(timestampBase - 10_000 - index * 1_000),
    ], env);
  });

  return env;
}

function newestReport(prefix, startedAtMs) {
  if (!prefix || !fs.existsSync(REPORT_DIR)) {
    return null;
  }
  const candidates = fs
    .readdirSync(REPORT_DIR, { withFileTypes: true })
    .filter(entry => entry.isFile() && entry.name.startsWith(prefix) && entry.name.endsWith('.json'))
    .map(entry => {
      const fullPath = path.join(REPORT_DIR, entry.name);
      const stat = fs.statSync(fullPath);
      return { fullPath, name: entry.name, mtimeMs: stat.mtimeMs };
    })
    .filter(entry => entry.mtimeMs >= startedAtMs - 1000)
    .sort((a, b) => b.mtimeMs - a.mtimeMs);
  return candidates[0] ?? null;
}

function formatMs(value) {
  return Number.isFinite(value) ? `${value.toFixed(1)}ms` : 'n/a';
}

function summarizeReport(file) {
  if (!file) {
    return null;
  }
  const report = JSON.parse(fs.readFileSync(file, 'utf8'));
  const sessionOpen = report.sessionOpen ?? {};
  const rapidTarget = report.rapidSwitchBreakdown?.target ?? {};
  const viewport = report.viewport ?? {};
  const visualStateSummary = report.visualStateSummary ?? {};
  return {
    clickToLatestTextMs: Number(
      report.clickToLatestAnswerTextVisibleMs ??
        report.clickToLatestVisibleMs ??
        rapidTarget.clickToLatestTextVisibleMs ??
        rapidTarget.clickToLatestVisibleMs
    ),
    latestFrameSinceHydrateMs: Number(
      sessionOpen.latestFrameSinceHydrateMs ??
        rapidTarget.latestFrameSinceHydrateMs
    ),
    latestVisibleRoundCount: Number(viewport.visibleModelRoundCount),
    postVisibleInteraction: report.postVisibleInteraction ?? 'none',
    visualBlankEvents: Number(
      visualStateSummary.postLatestTextVisibleBlankSurfacePointEventCount ??
        visualStateSummary.openIntentBlankSurfacePointEventCount ??
        visualStateSummary.blankSurfacePointEventCount
    ),
  };
}

function selectedScenarioNames() {
  const requested =
    argValue('--profile') ||
    argValue('--scenarios') ||
    process.env.BITFUN_E2E_PERF_MATRIX_PROFILE ||
    'core';
  const names = profiles[requested] ?? requested.split(',').map(name => name.trim()).filter(Boolean);
  const unknown = names.filter(name => !scenarios[name]);
  if (unknown.length > 0) {
    throw new Error(
      `Unknown long-session interaction scenario/profile: ${unknown.join(', ')}. ` +
        `Known profiles=${Object.keys(profiles).join(', ')} scenarios=${Object.keys(scenarios).join(', ')}`,
    );
  }
  return { requested, names };
}

function runScenario(name, baseEnv, options) {
  const scenario = scenarios[name];
  const env = {
    ...prepareScenarioRuntime(name, baseEnv),
    ...(scenario.env ?? {}),
  };
  const args = [
    '--dir',
    'tests/e2e',
    'exec',
    'wdio',
    'run',
    './config/wdio.conf.ts',
    '--spec',
    scenario.spec,
  ];
  if (scenario.grep) {
    args.push(`--mochaOpts.grep=${scenario.grep}`);
  }

  console.log(`[long-session-matrix] start scenario=${name}`);
  const startedAtMs = Date.now();
  const result = runPnpm(args, {
    cwd: ROOT,
    env,
    ...runnerStdioOptions(),
  });
  const reportEntry = newestReport(scenario.reportPrefix, startedAtMs);
  const summary = summarizeReport(reportEntry?.fullPath);
  const missingExpectedReport =
    Boolean(scenario.reportPrefix) && !reportEntry && !options.allowMissingReports;

  if (summary) {
    console.log(
      `[long-session-matrix] done scenario=${name} ` +
        `clickToLatestText=${formatMs(summary.clickToLatestTextMs)} ` +
        `latestFrame=${formatMs(summary.latestFrameSinceHydrateMs)} ` +
        `visibleRounds=${summary.latestVisibleRoundCount} ` +
        `postInteraction=${summary.postVisibleInteraction} ` +
        `blankEvents=${summary.visualBlankEvents} report=${reportEntry.name}`,
    );
  } else {
    console.log(`[long-session-matrix] done scenario=${name} report=none`);
  }

  return {
    name,
    ok: result.status === 0 && !missingExpectedReport,
    status: result.status,
    error:
      result.error?.message ??
      (missingExpectedReport
        ? 'expected performance report was not written; fixture may be missing or the spec skipped'
        : outputTail(result)),
    reportName: reportEntry?.name ?? null,
  };
}

const { requested, names } = selectedScenarioNames();
const missingReportsAllowed = allowMissingReports();
const baseEnv = {
  ...process.env,
  BITFUN_E2E_APP_MODE: process.env.BITFUN_E2E_APP_MODE || 'release-fast',
  E2E_LOG_LEVEL: process.env.E2E_LOG_LEVEL || 'warn',
};

if (hasFlag('--dry-run')) {
  console.log(
    `[long-session-matrix] dry-run profile=${requested} appMode=${baseEnv.BITFUN_E2E_APP_MODE} ` +
      `allowMissingReports=${missingReportsAllowed} scenarios=${names.join(',')}`,
  );
  process.exit(0);
}

pruneOldPerfRuns(Math.max(0, MAX_RETAINED_PERF_RUNS - 1));

const results = names.map(name =>
  runScenario(name, baseEnv, { allowMissingReports: missingReportsAllowed }),
);
const failed = results.filter(result => !result.ok);
if (failed.length > 0) {
  for (const failure of failed) {
    console.error(
      `[long-session-matrix] failed scenario=${failure.name} status=${failure.status} ` +
        `error=${failure.error ?? 'none'} report=${failure.reportName ?? 'none'}`,
    );
  }
  process.exit(1);
}

console.log(`[long-session-matrix] summary scenarios=${results.length} failed=0`);
