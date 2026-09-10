import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const repoRoot = path.resolve(import.meta.dirname, '../..');
const shell = `
set -euo pipefail
source "$RELEASE_SCRIPT"
source "$SOURCE_SCRIPT"
openbitfun_relay_native_platform() { echo linux/amd64; }
openbitfun_image_docker_with_timeout() { shift; openbitfun_image_docker "$@"; }
openbitfun_image_docker() {
  printf '%s\\n' "$*" >> "$CALLS"
  case "$1" in
    pull) [[ "$SCENARIO" = image_* ]] ;;
    build)
      if [ "$SCENARIO" = build_failed ]; then return 1; fi
      if [ "$SCENARIO" = cancelled ]; then kill -TERM "$(sh -c 'echo "$PPID"')"; return 1; fi
      while [ "$#" -gt 0 ]; do
        if [ "$1" = --iidfile ]; then printf 'sha256:%064d' 1 > "$2"; break; fi
        shift
      done
      ;;
    image) echo amd64 ;;
    container) return 0 ;; # An existing, healthy Relay must be preserved.
    exec)
      [ "$SCENARIO" != health_failed ] || return 1
      [ "$SCENARIO" != image_health_failed ] || [[ "$OPENBITFUN_RELAY_IMAGE" = source:* ]]
      ;;
    run)
      [ "$SCENARIO" != image_start_failed ] || [[ "$OPENBITFUN_RELAY_IMAGE" = source:* ]]
      ;;
    inspect) echo false ;;
    ps) return 0 ;;
    *) return 0 ;;
  esac
}
openbitfun_deploy_with_source_fallback "$MODE" "$SOURCE_ROOT"
echo RELAY_TASK_DONE
`;

function runScenario(t, scenario, mode = 'image') {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-source-build-'));
  t.after(() => fs.rmSync(temp, { recursive: true, force: true }));
  const upstream = path.join(temp, 'upstream');
  fs.mkdirSync(path.join(upstream, 'src/apps/relay-server'), { recursive: true });
  fs.writeFileSync(path.join(upstream, 'src/apps/relay-server/Dockerfile'), 'FROM scratch\n');
  for (const args of [
    ['init', '-q', '-b', 'main'], ['add', '.'],
    ['-c', 'user.name=Relay Test', '-c', 'user.email=relay@example.invalid', 'commit', '-qm', 'fixture'],
  ]) {
    const git = spawnSync('git', args, { cwd: upstream, encoding: 'utf8', windowsHide: true });
    assert.equal(git.status, 0, git.stderr);
  }
  const revision = spawnSync('git', ['rev-parse', 'HEAD'], {
    cwd: upstream, encoding: 'utf8', windowsHide: true,
  }).stdout.trim();
  const sourceRoot = path.join(temp, 'source with spaces');
  fs.mkdirSync(sourceRoot);
  fs.writeFileSync(path.join(sourceRoot, 'user-file'), 'keep me');
  const calls = path.join(temp, 'calls');
  fs.writeFileSync(calls, '');
  const result = spawnSync('bash', ['-c', shell], {
    encoding: 'utf8', windowsHide: true, timeout: 20_000,
    env: {
      ...process.env,
      RELEASE_SCRIPT: path.join(repoRoot, 'src/apps/relay-server/release-download.sh'),
      SOURCE_SCRIPT: path.join(repoRoot, 'src/apps/relay-server/source-build.sh'),
      SOURCE_ROOT: sourceRoot,
      OPENBITFUN_REPO_GIT_URL: scenario === 'download_failed' ? path.join(temp, 'missing') : upstream,
      // Exercise route fallback without making a network request.
      OPENBITFUN_GITHUB_GIT_URL: path.join(temp, 'unreachable-mirror'),
      OPENBITFUN_MIRROR_REQUESTED_MODE: 'global',
      OPENBITFUN_RELAY_IMAGE_DIGEST: `sha256:${'a'.repeat(64)}`,
      OPENBITFUN_REQUIRE_IMAGE_DIGEST: '1',
      MODE: mode, SCENARIO: scenario, CALLS: calls,
    },
  });
  assert.deepEqual(fs.readdirSync(sourceRoot), ['user-file'], 'clean only the task checkout');
  return { ...result, calls: fs.readFileSync(calls, 'utf8'), revision };
}

test('available image succeeds without fetching or building source', (t) => {
  const result = runScenario(t, 'image_ok');
  assert.equal(result.status, 0, result.stderr);
  assert.doesNotMatch(result.calls, /^build /m);
  assert.doesNotMatch(result.stdout, /Fetching current source/);
  assert.match(result.stdout, /RELAY_TASK_DONE/);
});

for (const mode of ['source', 'image']) {
  test(`${mode}: missing artifact or failed pull builds before replacing the existing Relay`, (t) => {
    const result = runScenario(t, 'source_ok', mode);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /falling back to a source build/);
    assert.ok(result.calls.indexOf('build ') < result.calls.indexOf('stop openbitfun-relay'));
    assert.match(result.calls, new RegExp(`--build-arg RELAY_GIT_COMMIT=${result.revision}`));
    assert.match(result.calls, /--build-arg CARGO_BUILD_JOBS=1/);
    assert.match(result.calls, /-v relay-server_relay-db:\/app\/data/);
    assert.match(result.calls, /run .*sha256:0{63}1\n/);
    assert.match(result.stdout, /RELAY_TASK_DONE/);
  });
}

for (const scenario of ['build_failed', 'download_failed', 'cancelled']) {
  test(`${scenario}: fail visibly without stopping the existing Relay`, (t) => {
    const result = runScenario(t, scenario, 'source');
    assert.notEqual(result.status, 0);
    assert.doesNotMatch(result.calls, /^(stop|rename|run|rm) /m);
    assert.doesNotMatch(result.stdout, /RELAY_TASK_DONE/);
  });
}

test('source health failure restores the previous container and reports failure', (t) => {
  const result = runScenario(t, 'health_failed', 'source');
  assert.notEqual(result.status, 0);
  assert.match(result.calls, /rename openbitfun-relay-before-image-\d+ openbitfun-relay/);
  assert.match(result.calls, /start openbitfun-relay/);
  assert.doesNotMatch(result.stdout, /RELAY_TASK_DONE/);
});

for (const scenario of ['image_start_failed', 'image_health_failed']) {
  test(`${scenario}: restore service before attempting the source fallback`, (t) => {
    const result = runScenario(t, scenario);
    assert.equal(result.status, 0, result.stderr);
    const restoration = result.calls.indexOf('start openbitfun-relay');
    assert.ok(restoration >= 0 && restoration < result.calls.indexOf('build '));
    assert.match(result.stdout, /RELAY_TASK_DONE/);
  });
}
