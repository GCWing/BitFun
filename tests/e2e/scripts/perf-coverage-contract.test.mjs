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
  assert.match(startupRunner, /BITFUN_E2E_PERF_STARTUP_MAX_INTERACTIVE_MS/);
  assert.match(startupRunner, /collects startup timing from the current build/);
  assert.match(startupRunner, /seenTraceIds/);
  assert.match(startupRunner, /was already reported by an earlier iteration/);

  const interactionRunner = readText('tests/e2e/scripts/run-long-session-interaction-matrix.mjs');
  assert.match(interactionRunner, /BITFUN_E2E_PERF_MATRIX_PROFILE/);
  assert.match(interactionRunner, /first-scroll/);
  assert.match(interactionRunner, /resize-window-width/);
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
