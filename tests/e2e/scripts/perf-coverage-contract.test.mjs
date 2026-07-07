import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..');

function readText(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), 'utf8');
}

function readJson(relativePath) {
  return JSON.parse(readText(relativePath));
}

test('performance scripts expose focused startup stability and interaction profiles', () => {
  const rootPackage = readJson('package.json');

  assert.match(
    rootPackage.scripts['e2e:test:perf:startup-stability:release-fast'] ?? '',
    /run-startup-stability\.mjs/,
  );
  assert.match(
    rootPackage.scripts['e2e:test:perf:long-session-interactions:release-fast'] ?? '',
    /run-long-session-interaction-matrix\.mjs/,
  );

  const startupRunner = readText('tests/e2e/scripts/run-startup-stability.mjs');
  assert.match(startupRunner, /BITFUN_E2E_PERF_STARTUP_ITERATIONS/);
  assert.match(startupRunner, /--samples/);
  assert.match(startupRunner, /--iterations/);
  assert.match(startupRunner, /readIntegerArgOrEnv/);
  assert.match(startupRunner, /BITFUN_E2E_PERF_STARTUP_MAX_INTERACTIVE_MS/);
  assert.match(startupRunner, /collects startup timing from the current build/);
  assert.match(startupRunner, /seenTraceIds/);
  assert.match(startupRunner, /was already reported by an earlier iteration/);

  const interactionRunner = readText('tests/e2e/scripts/run-long-session-interaction-matrix.mjs');
  assert.match(interactionRunner, /BITFUN_E2E_PERF_MATRIX_PROFILE/);
  assert.match(interactionRunner, /first-scroll/);
  assert.match(interactionRunner, /resize-window-width/);
  assert.match(interactionRunner, /turn-navigation/);
  assert.match(interactionRunner, /l1-chat-turn-navigation-release\.spec\.ts/);
  assert.match(interactionRunner, /BITFUN_E2E_PERF_RAPID_SWITCH_DELAY_MS/);
  assert.match(interactionRunner, /BITFUN_E2E_PERF_ALLOW_MISSING_REPORTS/);
  assert.match(interactionRunner, /expected performance report was not written/);
});

test('release-fast startup telemetry rejects dev-server contaminated samples', () => {
  const startupSpec = readText('tests/e2e/specs/performance/startup-session-perf.spec.ts');

  assert.match(startupSpec, /assertReleaseFastPerfRuntime/);
  assert.match(startupSpec, /release-fast perf run loaded a dev-server URL/);
  assert.match(startupSpec, /runtimeUrl/);
});

test('embedded startup probe uses short script timeouts while waiting for readiness', () => {
  const embeddedDriver = readText('tests/e2e/config/embedded-driver.ts');

  assert.match(embeddedDriver, /setProbeScriptTimeout/);
  assert.match(embeddedDriver, /\/session\/\$\{sessionId\}\/timeouts/);
  assert.match(embeddedDriver, /setProbeScriptTimeout\(sessionId, 1000\)/);
});

test('long session required frame trace samples fail when trace phases are missing', () => {
  const startupSpec = readText('tests/e2e/specs/performance/startup-session-perf.spec.ts');

  assert.match(startupSpec, /Long session measurement missing required trace phases/);
  assert.match(startupSpec, /measurement\.traceWaitErrors\.length > 0/);
  assert.match(startupSpec, /measurement\.clickToPostHydrateUsableMs\)\.toBeGreaterThan\(0\)/);
});

test('long session navigation lookup follows current session nav DOM contract', () => {
  const startupSpec = readText('tests/e2e/specs/performance/startup-session-perf.spec.ts');

  assert.match(startupSpec, /data-testid="nav-session-item"/);
  assert.match(startupSpec, /data-testid="nav-session-list-toggle"/);
  assert.match(startupSpec, /data-session-nav-toggle-action/);
  assert.match(startupSpec, /data-session-id/);
});

test('long session interaction matrix isolates user data and avoids active-session preload bias', () => {
  const interactionRunner = readText('tests/e2e/scripts/run-long-session-interaction-matrix.mjs');

  assert.match(interactionRunner, /BITFUN_E2E_STORAGE_ROOT/);
  assert.match(interactionRunner, /generate-long-session-fixture\.mjs/);
  assert.match(interactionRunner, /BITFUN_E2E_PERF_SESSION_ID/);
  assert.match(interactionRunner, /perf-long-session-001/);
  assert.match(interactionRunner, /BITFUN_E2E_PERF_RAPID_SWITCH_SESSION_IDS/);
  assert.match(interactionRunner, /perf-rapid-c-000/);
  assert.match(interactionRunner, /pruneOldPerfRuns/);
  assert.match(interactionRunner, /MAX_RETAINED_PERF_RUNS/);
});
