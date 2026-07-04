import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(SCRIPT_DIR, '..', '..', '..');
const REPORT_DIR = path.join(ROOT, 'tests', 'e2e', 'reports', 'performance');
const STARTUP_TEST_NAME = 'collects startup timing from the current build';

function readFlag(name) {
  return process.argv.includes(name);
}

function readArgValue(name) {
  const index = process.argv.indexOf(name);
  if (index < 0) {
    return undefined;
  }
  return process.argv[index + 1];
}

function parseNonNegativeNumber(name, raw) {
  const value = Number(raw);
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`${name} must be a non-negative number, got ${raw}`);
  }
  return value;
}

function readNumberEnv(name, fallback) {
  const raw = process.env[name];
  if (raw === undefined || raw === '') {
    return fallback;
  }
  return parseNonNegativeNumber(name, raw);
}

function readNumberArgOrEnv(argNames, envName, fallback) {
  for (const argName of argNames) {
    const raw = readArgValue(argName);
    if (raw !== undefined && raw !== '') {
      return parseNonNegativeNumber(argName, raw);
    }
  }
  return readNumberEnv(envName, fallback);
}

function readIntegerArgOrEnv(argNames, envName, fallback) {
  return Math.max(1, Math.trunc(readNumberArgOrEnv(argNames, envName, fallback)));
}

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

function formatMs(value) {
  return Number.isFinite(value) ? `${value.toFixed(1)}ms` : 'n/a';
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function newestReport(prefix, startedAtMs) {
  if (!fs.existsSync(REPORT_DIR)) {
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

function summarizeReport(report) {
  const startup = report.startup ?? {};
  const firstScriptEvalMs = Number(startup.firstScriptEvalMs);
  const mainWindowShownMs = Number(startup.mainWindowShownMs);
  const interactiveShellReadyMs = Number(startup.interactiveShellReadyMs);
  const mainShownToInteractiveMs =
    Number.isFinite(interactiveShellReadyMs) && Number.isFinite(mainWindowShownMs)
      ? interactiveShellReadyMs - mainWindowShownMs
      : Number.NaN;

  return {
    traceId: report.traceId ?? 'unknown',
    runtimeUrl: report.runtimeUrl ?? 'unknown',
    runtimeHostname: report.runtimeHostname ?? 'unknown',
    firstScriptEvalMs,
    mainWindowShownMs,
    interactiveShellReadyMs,
    mainShownToInteractiveMs,
    setupTrayBuildMs: Number(report.breakdown?.native?.setupSteps?.['setup_tray.build']),
  };
}

function checkThresholds(summary, thresholds) {
  const failures = [];
  for (const field of [
    'interactiveShellReadyMs',
    'firstScriptEvalMs',
    'mainShownToInteractiveMs',
  ]) {
    if (!Number.isFinite(summary[field])) {
      failures.push(`${field}=n/a`);
    }
  }
  if (summary.interactiveShellReadyMs > thresholds.interactiveShellReadyMs) {
    failures.push(
      `interactive=${formatMs(summary.interactiveShellReadyMs)} > ${formatMs(thresholds.interactiveShellReadyMs)}`,
    );
  }
  if (summary.firstScriptEvalMs > thresholds.firstScriptEvalMs) {
    failures.push(
      `firstScript=${formatMs(summary.firstScriptEvalMs)} > ${formatMs(thresholds.firstScriptEvalMs)}`,
    );
  }
  if (summary.mainShownToInteractiveMs > thresholds.mainShownToInteractiveMs) {
    failures.push(
      `shownToInteractive=${formatMs(summary.mainShownToInteractiveMs)} > ${formatMs(thresholds.mainShownToInteractiveMs)}`,
    );
  }
  return failures;
}

function runStartupIteration(index, total, env, thresholds, seenTraceIds) {
  const startedAtMs = Date.now();
  const result = runPnpm(
    [
      '--dir',
      'tests/e2e',
      'exec',
      'wdio',
      'run',
      './config/wdio.conf.ts',
      '--spec',
      './specs/performance/startup-session-perf.spec.ts',
      `--mochaOpts.grep=${STARTUP_TEST_NAME}`,
    ],
    {
      cwd: ROOT,
      env,
      ...runnerStdioOptions(),
    },
  );

  const reportEntry = newestReport('startup-', startedAtMs);
  if (result.status !== 0) {
    const tail = outputTail(result);
    return {
      ok: false,
      error: result.error
        ? `wdio failed to spawn: ${result.error.message}`
        : `wdio exited with ${result.status}${tail ? `\n${tail}` : ''}`,
      reportName: reportEntry?.name ?? null,
    };
  }
  if (!reportEntry) {
    return {
      ok: false,
      error: 'startup report was not written',
      reportName: null,
    };
  }

  const summary = summarizeReport(readJson(reportEntry.fullPath));
  const thresholdFailures = checkThresholds(summary, thresholds);
  if (!summary.traceId || summary.traceId === 'unknown') {
    thresholdFailures.push('traceId=missing');
  } else if (seenTraceIds.has(summary.traceId)) {
    thresholdFailures.push(`traceId=${summary.traceId} was already reported by an earlier iteration`);
  } else {
    seenTraceIds.add(summary.traceId);
  }
  const line =
    `[startup-stability] ${index}/${total} ` +
    `interactive=${formatMs(summary.interactiveShellReadyMs)} ` +
    `firstScript=${formatMs(summary.firstScriptEvalMs)} ` +
    `shownToInteractive=${formatMs(summary.mainShownToInteractiveMs)} ` +
    `trayBuild=${formatMs(summary.setupTrayBuildMs)} ` +
    `trace=${summary.traceId} runtime=${summary.runtimeHostname} report=${reportEntry.name}`;
  console.log(line);

  return {
    ok: thresholdFailures.length === 0,
    error: thresholdFailures.join('; '),
    reportName: reportEntry.name,
    summary,
  };
}

const iterations = readIntegerArgOrEnv(
  ['--samples', '--iterations'],
  'BITFUN_E2E_PERF_STARTUP_ITERATIONS',
  5,
);
const thresholds = {
  interactiveShellReadyMs: readNumberEnv('BITFUN_E2E_PERF_STARTUP_MAX_INTERACTIVE_MS', 5000),
  firstScriptEvalMs: readNumberEnv('BITFUN_E2E_PERF_STARTUP_MAX_FIRST_SCRIPT_MS', 3000),
  mainShownToInteractiveMs: readNumberEnv(
    'BITFUN_E2E_PERF_STARTUP_MAX_MAIN_SHOWN_TO_INTERACTIVE_MS',
    3000,
  ),
};
const env = {
  ...process.env,
  BITFUN_E2E_APP_MODE: process.env.BITFUN_E2E_APP_MODE || 'release-fast',
  E2E_LOG_LEVEL: process.env.E2E_LOG_LEVEL || 'warn',
};

if (readFlag('--dry-run')) {
  console.log(
    `[startup-stability] dry-run iterations=${iterations} appMode=${env.BITFUN_E2E_APP_MODE} ` +
      `grep="${STARTUP_TEST_NAME}"`,
  );
  process.exit(0);
}

const results = [];
const seenTraceIds = new Set();
for (let index = 1; index <= iterations; index += 1) {
  results.push(runStartupIteration(index, iterations, env, thresholds, seenTraceIds));
}

const failed = results.filter(result => !result.ok);
const summaries = results.map(result => result.summary).filter(Boolean);
if (summaries.length > 0) {
  const interactiveValues = summaries
    .map(summary => summary.interactiveShellReadyMs)
    .filter(Number.isFinite)
    .sort((a, b) => a - b);
  const min = interactiveValues[0];
  const max = interactiveValues[interactiveValues.length - 1];
  const median = interactiveValues[Math.floor(interactiveValues.length / 2)];
  console.log(
    `[startup-stability] summary samples=${interactiveValues.length} ` +
      `interactiveMedian=${formatMs(median)} min=${formatMs(min)} max=${formatMs(max)}`,
  );
}

if (failed.length > 0) {
  for (const failure of failed) {
    console.error(
      `[startup-stability] failed report=${failure.reportName ?? 'none'} reason=${failure.error}`,
    );
  }
  process.exit(1);
}
