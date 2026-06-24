import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(SCRIPT_DIR, '..', '..', '..');
const REPORT_DIR = path.join(ROOT, 'tests', 'e2e', 'reports', 'performance');

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
  core: ['first-open', 'rapid-switch-zero-delay', 'first-scroll', 'resize-window-width'],
  scroll: ['first-scroll', 'scroll-down'],
  resize: ['resize-window', 'resize-window-width'],
  full: [
    'first-open',
    'warm-reopen',
    'rapid-switch-zero-delay',
    'first-scroll',
    'scroll-down',
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
    ...baseEnv,
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
