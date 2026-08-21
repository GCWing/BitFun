// bitfun-loopx MiniApp — Worker (Node.js/Bun).
// Wraps the loopx CLI (`loopx --format json …`). The host heartbeat lives in
// ui.js; this worker is stateless per call except for the cached invocation
// prefix and the per-goal run-once in-flight registry.

const { spawn } = require('child_process');
const fs = require('fs');
const https = require('https');
const os = require('os');
const path = require('path');

// Invocation candidates, probed in order. `python -m loopx` does NOT work
// (loopx has no __main__.py) — the module form must target loopx.cli.
// The worker process may inherit a restricted PATH (the host app can be
// launched from an environment without the Python Scripts dir), so common
// absolute locations of the pip console-script are probed directly too.
// Absolute python.exe locations (robust against a restricted worker PATH).
function absolutePythonExes() {
  const list = [];
  const localAppData = process.env.LOCALAPPDATA;
  if (localAppData) {
    const pyRoot = path.join(localAppData, 'Programs', 'Python');
    let versions = [];
    try {
      versions = fs.readdirSync(pyRoot).filter((name) => /^Python\d+$/.test(name));
    } catch (_) {}
    for (const version of versions) {
      const exe = path.join(pyRoot, version, 'python.exe');
      if (fs.existsSync(exe)) list.push(exe);
    }
  } else if (process.platform !== 'win32') {
    // Common absolute interpreter locations so a restricted worker PATH
    // cannot hide a usable Python on macOS/Linux.
    for (const exe of [
      '/opt/homebrew/bin/python3',
      '/usr/local/bin/python3',
      '/usr/bin/python3',
      '/opt/homebrew/bin/python',
      '/usr/local/bin/python',
      '/usr/bin/python',
    ]) {
      if (fs.existsSync(exe)) list.push(exe);
    }
  }
  return list;
}

// Interpreter argv prefixes for `-m loopx.cli`, in probe order. `py -3` is the
// Windows launcher; POSIX systems name the 3.x interpreter `python3` first.
function moduleInterpreterArgvs() {
  return process.platform === 'win32'
    ? [['python'], ['py', '-3']]
    : [['python3'], ['python']];
}

// Bundled sidecar binary shipped with the desktop installer (build-time
// PyInstaller onefile; see scripts/build-loopx.mjs). The host passes the
// resources directory through BITFUN_RESOURCE_DIR. When present this binary
// needs no Python, no git and no network — it outranks every other candidate.
function bundledLoopxExes() {
  const dir = process.env.BITFUN_RESOURCE_DIR;
  if (!dir) return [];
  const names = process.platform === 'win32' ? ['loopx.exe', 'loopx'] : ['loopx', 'loopx.exe'];
  return names
    .map((name) => path.join(dir, 'loopx', name))
    .filter((candidate) => fs.existsSync(candidate));
}

function candidatePrefixes(srcDir) {
  const list = [
    ...bundledLoopxExes().map((exe) => ({ argv: [exe] })),
    { argv: ['loopx'] },
  ];
  for (const argv of moduleInterpreterArgvs()) {
    list.push({ argv: [...argv, '-m', 'loopx.cli'] });
  }
  for (const exe of absolutePythonExes()) {
    const loopxExe = path.join(path.dirname(exe), 'Scripts', 'loopx.exe');
    if (fs.existsSync(loopxExe)) list.push({ argv: [loopxExe] });
  }
  // Source-checkout candidates. Both the user-configured srcDir and the
  // console's own vendored checkout are valid; the vendor dir is probed even
  // when srcDir is empty, so a previously-vendored loopx is always found
  // without any configuration (loopx is pure stdlib, Python >= 3.11).
  const srcDirs = [...new Set([srcDir, vendorLoopxDir()].filter(Boolean))];
  for (const dir of srcDirs) {
    if (!fs.existsSync(path.join(dir, 'loopx', 'cli.py'))) continue;
    for (const argv of moduleInterpreterArgvs()) {
      list.push({ argv: [...argv, '-m', 'loopx.cli'], env: { PYTHONPATH: dir } });
    }
    for (const exe of absolutePythonExes()) {
      list.push({ argv: [exe, '-m', 'loopx.cli'], env: { PYTHONPATH: dir } });
    }
  }
  return list;
}

const DEFAULT_TIMEOUT_MS = 180000;

// PR identity markers — the countability contract: every PR created by this
// tool carries both keywords, searchable on GitHub with `"bitfun-loopx" in:title`.
const PR_TITLE_PREFIX = '[bitfun-loopx] ';
const PR_BODY_MARKER = 'Created by bitfun-loopx (BitFun built-in MiniApp).';

// ── debug trace (worker side) ─────────────────────────────────
// Written to <appdir>/debug-worker.log so host logs are not required.
const DEBUG_WORKER = [];
function dbgWorker(tag, detail) {
  const line = `${new Date().toISOString()} ${tag} ${detail || ''}`;
  DEBUG_WORKER.push(line);
  if (DEBUG_WORKER.length > 200) DEBUG_WORKER.shift();
  try {
    fs.writeFileSync(path.join(process.cwd(), 'debug-worker.log'), `${DEBUG_WORKER.join('\n')}\n`);
  } catch (_) {}
}
dbgWorker('boot', `pid=${process.pid} runtime=${process.version}`);

let cachedPrefix = null; // { argv, env }

// loopx is a Python CLI; on zh-CN Windows its stdout defaults to GBK, which
// breaks both JSON.parse and any non-ASCII reason strings. Force UTF-8, and
// prepend (never clobber) PYTHONPATH from a source-checkout overlay.
function buildEnv(envOverlay) {
  const env = { ...process.env, PYTHONUTF8: '1', PYTHONIOENCODING: 'utf-8' };
  if (envOverlay) {
    for (const [key, value] of Object.entries(envOverlay)) {
      if (key === 'PYTHONPATH' && env.PYTHONPATH) {
        env.PYTHONPATH = value + path.delimiter + env.PYTHONPATH;
      } else {
        env[key] = value;
      }
    }
  }
  return env;
}

// child.kill() only terminates the direct child; on Windows the interesting
// work (python → codex …) lives in grandchildren. Kill the whole tree: on
// POSIX the child is spawned detached (own process group), so signal the
// group; on Windows use taskkill /T.
function killTree(child) {
  if (!child || child.killed || child.exitCode !== null) return;
  if (process.platform === 'win32') {
    try {
      const tk = spawn('taskkill', ['/pid', String(child.pid), '/T', '/F'], { windowsHide: true });
      tk.on('error', () => child.kill());
    } catch (_) {
      child.kill();
    }
  } else {
    try {
      process.kill(-child.pid, 'SIGKILL');
    } catch (_) {
      child.kill();
    }
  }
}

function makeLineSplitter(onLine) {
  let buf = '';
  return {
    push(text) {
      buf += text;
      let idx;
      while ((idx = buf.indexOf('\n')) >= 0) {
        const line = buf.slice(0, idx).replace(/\r$/, '');
        buf = buf.slice(idx + 1);
        if (line.trim()) onLine(line);
      }
    },
    flush() {
      if (buf.trim()) onLine(buf.trim());
      buf = '';
    },
  };
}

// onStderrLine streams progress (loopx logs to stderr while running; in
// --format json mode stdout carries exactly one JSON document at exit, so
// streaming stdout lines would only replay the final payload as noise).
function spawnLoopx(prefix, args, { timeoutMs = DEFAULT_TIMEOUT_MS, onStderrLine = null, onSpawned = null } = {}) {
  const argv = Array.isArray(prefix) ? prefix : prefix.argv;
  const envOverlay = Array.isArray(prefix) ? null : prefix.env;
  return new Promise((resolve, reject) => {
    const [cmd, ...prefixArgs] = argv;
    const child = spawn(cmd, [...prefixArgs, ...args], {
      shell: false,
      windowsHide: true,
      // POSIX: run the child in its own process group so killTree can signal
      // the whole loopx tree on timeout (kill() alone would leak python/codex
      // descendants).
      detached: process.platform !== 'win32',
      env: buildEnv(envOverlay),
    });
    if (onSpawned) onSpawned(child);
    let stdout = '';
    let stderr = '';
    // setEncoding routes chunks through a StringDecoder, so multibyte UTF-8
    // sequences split across 64KB pipe chunks decode correctly (quota status
    // payloads run to hundreds of KB with CJK text).
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    const errSplitter = onStderrLine ? makeLineSplitter(onStderrLine) : null;
    let settled = false;
    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      killTree(child);
      reject(new Error(`loopx timed out after ${timeoutMs}ms: ${args.join(' ')}`));
    }, timeoutMs);

    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
      if (errSplitter) errSplitter.push(chunk);
    });
    child.on('error', (err) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject(err);
    });
    child.on('close', (code) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (errSplitter) errSplitter.flush();
      resolve({ code, stdout, stderr });
    });
  });
}

// loopx prints one JSON document on stdout in --format json mode; tolerate
// noise before the document and (best-effort) after it.
function parseJsonPayload(stdout) {
  const text = stdout.trim();
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch (_) {}
  const start = text.indexOf('{');
  if (start >= 0) {
    try {
      return JSON.parse(text.slice(start));
    } catch (_) {}
    const end = text.lastIndexOf('}');
    if (end > start) {
      try {
        return JSON.parse(text.slice(start, end + 1));
      } catch (_) {}
    }
  }
  return null;
}

// A prefix from the UI may be a plain argv array (settings input) or a
// {argv, env} object persisted from a previous detect.
function normalizePrefix(p) {
  if (!p) return null;
  if (Array.isArray(p)) return p.length > 0 ? { argv: p } : null;
  if (typeof p === 'object' && Array.isArray(p.argv) && p.argv.length > 0) return p;
  return null;
}

async function resolvePrefix(argvPrefix, srcDir = null) {
  const normalized = normalizePrefix(argvPrefix);
  if (normalized) return normalized;
  if (cachedPrefix) return cachedPrefix;
  const detected = await detectLoopx(null, srcDir);
  if (!detected.found) {
    throw new Error(
      'loopx CLI not found. Install it (pip install -e <checkout>), '
      + 'or set the invocation command or source directory in Settings.'
    );
  }
  return normalizePrefix(detected.argvPrefix) || { argv: detected.argvPrefix };
}

function registryArgs(projectDir) {
  if (!projectDir) return []; // loopx falls back to its global registry
  return ['--registry', path.join(projectDir, '.loopx', 'registry.json')];
}

// loopx quota commands run a "public boundary" leak scan whose default root is
// the loopx source checkout itself; on a cold cache that scan alone can take
// over a minute. The monitoring gate never consumes scan hits, so point the
// scan at a small dedicated directory instead and keep the heartbeat fast.
function quotaScanRoot() {
  const dir = path.join(process.cwd(), '.loopx-scan-root');
  try { fs.mkdirSync(dir, { recursive: true }); } catch (_) {}
  return dir;
}

// ── auto-clone (direction C, docs/product-spec.md) ──────────────
// When no local checkout is selected, the target repository is cloned into the
// MiniApp's own data directory and the goal binds to that clone. Progress is
// streamed through the taskIntake event channel (stage 'clone').
//
// The clone cache lives under the USER's stable .bitfun directory, NOT the
// per-instance appdata: deleting/re-importing the MiniApp would otherwise
// wipe every clone and force a fresh network clone. A stable cache also lets
// a fresh import rediscover the repos and their per-project registries.
// ── console data home ──
// The clone cache and the loopx vendor checkout live under the user's stable
// .bitfun directory, NOT the per-instance appdata: deleting/re-importing the
// MiniApp would otherwise wipe every clone and force a fresh network clone.
// A stable cache also lets a fresh import rediscover the repos and their
// per-project registries.
function consoleHome() {
  return path.join(os.homedir(), '.bitfun', 'bitfun-loopx');
}

function cloneCacheRoot() {
  return path.join(consoleHome(), 'repos');
}

function cloneTargetDir(repo) {
  const safe = repo.replace(/[^A-Za-z0-9._-]/g, '-');
  return path.join(cloneCacheRoot(), safe);
}

// ── loopx vendor copy (universal acquisition) ──────────────
// loopx has zero runtime dependencies (pure stdlib, requires Python >= 3.11),
// so a source checkout under the user's stable .bitfun directory runs directly
// via `python -m loopx.cli` + PYTHONPATH — no pip, no global install. Same
// stability rationale as the clone cache: re-importing the MiniApp must never
// wipe it, and a fresh import can rediscover it.
const LOOPX_REPO_URL = 'https://github.com/huangruiteng/loopx.git';
// Version pin for the vendored checkout. loopx's CLI JSON contract IS this
// app's interface surface (AGENTS.md §1): pulling latest main risks semantic
// drift. Move this pin only together with a deliberate contract upgrade of
// the console (and update the installed-loopx expectations at the same time).
const LOOPX_VENDOR_REF = 'v0.2.13';
const LOOPX_MIN_PYTHON = { major: 3, minor: 11 };

function vendorLoopxDir() {
  return path.join(consoleHome(), 'vendor', 'loopx');
}

function parsePythonVersion(text) {
  const m = String(text).match(/Python\s+(\d+)\.(\d+)/);
  if (!m) return null;
  return {
    major: Number(m[1]),
    minor: Number(m[2]),
    raw: String(text).trim().split(/\r?\n/)[0] || '',
  };
}

function pythonMeetsMinimum(version) {
  return !!(
    version
    && (version.major > LOOPX_MIN_PYTHON.major
      || (version.major === LOOPX_MIN_PYTHON.major && version.minor >= LOOPX_MIN_PYTHON.minor))
  );
}

// First interpreter that passes the version gate wins; otherwise report the
// first interpreter found at all so the UI can say "Python 3.9 detected,
// 3.11+ required" instead of a bare "not found".
async function probePython() {
  const attempts = [
    ...absolutePythonExes().map((exe) => ({ prefix: [exe], args: ['--version'] })),
  ];
  for (const argv of moduleInterpreterArgvs()) {
    attempts.push({ prefix: argv, args: ['--version'] });
  }
  let firstFound = null;
  for (const attempt of attempts) {
    let info;
    try {
      const { code, stdout, stderr } = await spawnLoopx({ argv: attempt.prefix }, attempt.args, { timeoutMs: 8000 });
      const version = parsePythonVersion(stdout + stderr);
      info = { found: code === 0 && !!version, version: version ? version.raw : null, ok: code === 0 && pythonMeetsMinimum(version) };
    } catch (_) {
      info = { found: false, version: null, ok: false };
    }
    if (info.ok) return { ...info, exe: attempt.prefix.join(' ') };
    if (!firstFound && info.found) firstFound = { ...info, exe: attempt.prefix.join(' ') };
  }
  return firstFound || { found: false, version: null, ok: false, exe: null };
}

async function probeGit() {
  try {
    const { code, stdout, stderr } = await spawnLoopx({ argv: ['git'] }, ['--version'], { timeoutMs: 8000 });
    return { found: code === 0, version: (stdout + stderr).trim().split(/\r?\n/)[0] || null };
  } catch (_) {
    return { found: false, version: null };
  }
}

function prereqErrorMessage(prereqs) {
  const missing = [];
  const python = prereqs && prereqs.python;
  if (!python || !python.ok) {
    missing.push(python && python.found && python.version
      ? `Python ${python.version} is too old (>= ${LOOPX_MIN_PYTHON.major}.${LOOPX_MIN_PYTHON.minor} required)`
      : 'Python >= 3.11 not found');
  }
  if (!prereqs || !prereqs.git || !prereqs.git.found) missing.push('git not found');
  return `missing prerequisites: ${missing.join(', ')}`;
}

async function checkPrereqs() {
  const python = await probePython();
  const git = await probeGit();
  return { ready: !!(python.ok && git.found), python, git };
}

// ── commit marker ──────────────────────────────────────────
// Every commit the agent makes inside a console-managed clone carries the
// bitfun-loopx co-author trailer, so the tool's commits are countable on
// GitHub with the commit-search query "Co-authored-by: bitfun-loopx".
// Two layers: a commit-msg hook (hard guarantee, installed in the clone)
// plus a prompt instruction (the agent must not remove the trailer).
const COMMIT_TRAILER_KEY = 'Co-authored-by: bitfun-loopx';
const COMMIT_TRAILER = `${COMMIT_TRAILER_KEY} <bitfun-loopx@users.noreply.github.com>`;
const COMMIT_MSG_HOOK = `#!/bin/sh
# bitfun-loopx marker: append the co-author trailer when absent.
TRAILER='${COMMIT_TRAILER}'
if ! grep -qF '${COMMIT_TRAILER_KEY}' "$1"; then
  printf '\\n%s\\n' "$TRAILER" >> "$1"
fi
`;

function ensureCommitTrailerHook(projectDir) {
  // Only console-managed clones get a hook installed — never a repo the user
  // selected on their own machine.
  if (!projectDir || !String(projectDir).startsWith(cloneCacheRoot())) return false;
  try {
    const hooksDir = path.join(projectDir, '.git', 'hooks');
    const hookPath = path.join(hooksDir, 'commit-msg');
    if (fs.existsSync(hookPath)) {
      const current = fs.readFileSync(hookPath, 'utf8');
      if (current.includes(COMMIT_TRAILER_KEY)) return true;
    }
    fs.mkdirSync(hooksDir, { recursive: true });
    fs.writeFileSync(hookPath, COMMIT_MSG_HOOK, { encoding: 'utf8', mode: 0o755 });
    dbgWorker('commitHook:installed', projectDir);
    return true;
  } catch (err) {
    dbgWorker('commitHook:error', `${err.message}`);
    return false;
  }
}

function repoExistsOnGithub(repo) {
  return new Promise((resolve, reject) => {
    const req = https.get(`https://api.github.com/repos/${repo}`, {
      headers: {
        'User-Agent': 'bitfun-loopx',
        Accept: 'application/vnd.github+json',
      },
      timeout: 30000,
    }, (res) => {
      res.resume();
      if (res.statusCode === 200) return resolve(true);
      if (res.statusCode === 404) return resolve(false);
      // Rate limits / server errors: let the clone attempt decide.
      resolve(true);
    });
    req.on('timeout', () => req.destroy(new Error('repository lookup timed out')));
    req.on('error', reject);
  });
}

async function cloneRepository(repo, emit) {
  const target = cloneTargetDir(repo);
  if (fs.existsSync(path.join(target, '.git'))) {
    emit({ detail: 'cached' });
    return target;
  }
  fs.mkdirSync(path.dirname(target), { recursive: true });
  const url = `https://github.com/${repo}.git`;
  emit({ detail: 'start', url });
  await new Promise((resolve, reject) => {
    const child = spawn('git', ['clone', '--progress', url, target], { windowsHide: true });
    let lastPercent = -1;
    const onData = (chunk) => {
      for (const rawLine of String(chunk).split('\n')) {
        const line = rawLine.trim();
        if (!line) continue;
        const pct = line.match(/Receiving objects:\s+(\d+)%/);
        if (pct && Number(pct[1]) !== lastPercent) {
          lastPercent = Number(pct[1]);
          emit({ percent: lastPercent });
        } else {
          emit({ detail: line.slice(0, 140) });
        }
      }
    };
    child.stdout.on('data', onData);
    child.stderr.on('data', onData);
    child.on('error', (err) => {
      try { fs.rmSync(target, { recursive: true, force: true }); } catch (_) {}
      reject(err);
    });
    child.on('close', (code) => {
      if (code === 0) resolve();
      else {
        try { fs.rmSync(target, { recursive: true, force: true }); } catch (_) {}
        reject(new Error(`git clone failed (exit ${code})`));
      }
    });
  });
  return target;
}

async function runJson(argvPrefix, projectDir, args, opts = {}) {
  const prefix = await resolvePrefix(argvPrefix, opts.srcDir || null);
  let result;
  try {
    result = await spawnLoopx(prefix, ['--format', 'json', ...registryArgs(projectDir), ...args], opts);
  } catch (err) {
    // A cached prefix can go stale (venv removed, PATH change): drop it so
    // the next call re-probes instead of failing forever.
    if (err && err.code === 'ENOENT' && prefix === cachedPrefix) cachedPrefix = null;
    throw err;
  }
  const payload = parseJsonPayload(result.stdout);
  return { result, payload, prefix };
}

async function detectLoopx(customPrefix, srcDir = null) {
  const custom = normalizePrefix(customPrefix);
  const candidates = custom ? [custom, ...candidatePrefixes(srcDir)] : candidatePrefixes(srcDir);
  const probes = [];
  for (const prefix of candidates) {
    try {
      const { code, stdout, stderr } = await spawnLoopx(prefix, ['--version'], { timeoutMs: 8000 });
      const version = (stdout + stderr).trim().split(/\r?\n/)[0] || '';
      const label = prefix.env ? [...prefix.argv, `(PYTHONPATH=${prefix.env.PYTHONPATH})`] : prefix.argv;
      probes.push({ argvPrefix: label, ok: code === 0, version });
      if (code === 0) {
        cachedPrefix = prefix;
        return { found: true, argvPrefix: prefix, version, probes };
      }
    } catch (err) {
      dbgWorker('detect:probeError', `${prefix.argv.join(' ')}: ${String(err.message || err)}`);
      probes.push({ argvPrefix: prefix.argv, ok: false, error: String(err.message || err) });
    }
  }
  return { found: false, argvPrefix: null, version: null, probes };
}

// Absolute python.exe locations (robust against a restricted worker PATH).
function pythonCandidates() {
  const list = [...absolutePythonExes()];
  list.push(...(process.platform === 'win32' ? ['python'] : ['python3', 'python']));
  return list;
}

function resolveRegistryPath(projectDir) {  if (projectDir) return path.join(projectDir, '.loopx', 'registry.json');
  // Mirror the CLI: LOOPX_REGISTRY wins only if the file exists; otherwise
  // loopx itself falls back to the global registry.
  if (process.env.LOOPX_REGISTRY && fs.existsSync(process.env.LOOPX_REGISTRY)) {
    return process.env.LOOPX_REGISTRY;
  }
  return path.join(os.homedir(), '.codex', 'loopx', 'registry.global.json');
}

function readRegistry(projectDir) {
  const registryPath = resolveRegistryPath(projectDir);
  try {
    return { registryPath, registry: JSON.parse(fs.readFileSync(registryPath, 'utf8')) };
  } catch (_) {
    return { registryPath, registry: null };
  }
}

function normalizeScheduler(schedulerHint) {
  if (!schedulerHint || typeof schedulerHint !== 'object') return null;
  // Hot path: reset token + codex_app fallback. Cold path
  // (--include-scheduler-detail): the local_scheduler block the heartbeat
  // actually wants.
  const cold = schedulerHint.cold_path_detail && schedulerHint.cold_path_detail.local_scheduler;
  const codexApp = schedulerHint.codex_app || {};
  const local = cold || {};
  const resetPolicy = schedulerHint.reset_policy || {};
  const unchangedPoll = schedulerHint.unchanged_poll || {};
  const limits = unchangedPoll.limits || {};
  const afterLimits = unchangedPoll.after_limits || {};
  return {
    recommendedIntervalMinutes:
      local.recommended_interval_minutes ?? codexApp.recommended_interval_minutes ?? null,
    maxIntervalMinutes: local.max_interval_minutes ?? codexApp.max_interval_minutes ?? null,
    backoffMultiplier: local.unchanged_poll_backoff_multiplier ?? null,
    unchangedPollLimit: local.unchanged_poll_limit ?? limits.local_scheduler ?? null,
    afterLimit: local.after_limit ?? afterLimits.local_scheduler ?? null,
    exampleProgression: local.example_progression_minutes ?? null,
    resetToken: resetPolicy.reset_token ?? null,
    cadenceClass: schedulerHint.cadence_class ?? null,
    action: schedulerHint.action ?? null,
    unchangedIdentityKeys: Array.isArray(schedulerHint.unchanged_identity_keys)
      ? schedulerHint.unchanged_identity_keys
      : null,
    source: cold ? 'local_scheduler' : 'codex_app_fallback',
  };
}

// Turn execution is delegated to the HOST's own agent (app.agent.run in the
// UI): loopx generates the per-turn task body via heartbeat-prompt, and this
// preamble binds it to the selected local checkout. No external CLI host
// (codex etc.) is involved — the user configures nothing.
function turnPreamble({ projectDir, goalId, agentId }) {
  const registry = path.join(projectDir, '.loopx', 'registry.json');
  const moduleFallback = `${moduleInterpreterArgvs()[0].join(' ')} -m loopx.cli`;
  const probeCmd = process.platform === 'win32' ? 'where loopx' : 'which loopx';
  const shellNote = process.platform === 'win32'
    ? '环境说明：命令在 PowerShell 中运行，不是 bash。loopx 示例命令里以 --turn-instance-id "${LOOPX_TURN:?}" 结尾的写法是 bash 语法 —— 不要照抄；--turn-instance-id 是可选参数，直接省略。'
    : '环境说明：命令在 bash 中运行。loopx 示例命令里的 ${LOOPX_TURN:?} 写法仅适用于 bash；--turn-instance-id 是可选参数，直接省略。';
  return [
    `你是 LoopX 目标 "${goalId}" 的执行 agent（agent_id: ${agentId}）。`,
    `仓库目录：${projectDir}`,
    `LoopX 注册表：${registry} —— 调用 loopx 命令时始终带上 --registry "${registry}"。`,
    `loopx CLI 已安装并位于 PATH（备用方式：${moduleFallback}）；每轮最多执行一次 "loopx --version"、"${probeCmd}" 之类的探测。`,
    shellNote,
    '规则：只在仓库目录内工作；不要强杀不是你启动的进程；',
    '在结束本轮前，通过 loopx（todo complete / 证据记录）记录进度。',
    '如果 loopx 命令失败，先读错误信息再修正调用方式 —— 同一命令不要盲目重试超过两次。',
    `提交规范：每次 git commit 的提交信息末尾必须保留一行 "${COMMIT_TRAILER}"（仓库已安装 commit-msg 钩子会自动补上；不要删除这一行）。`,
    '工作语言：所有面向用户的说明、总结与回复请使用中文。',
    '',
  ].join('\n');
}

// Strict intake grammar (product spec, docs/product-spec.md): the only
// supported GitHub links are
//   - a single item:  github.com/<owner>/<repo>/issues/<n> | /pull/<n>
//   - the issues list: github.com/<owner>/<repo>/issues
//   - the repository home: github.com/<owner>/<repo>
// Every other GitHub path (org pages, /settings, /pulls, /releases, /tree/...,
// /issues/new, commits, search, ...) is reported as unsupported instead of
// being silently treated as the repository. Extra text after the link stays
// allowed and becomes fix-instruction preamble.
function githubReferences(text) {
  const refs = [];
  const unsupported = [];
  const seen = new Set();
  const pattern = /https:\/\/github\.com\/[A-Za-z0-9._~:/?#[\]@!$&'()*+,;=%-]+/gi;
  for (const match of String(text || '').matchAll(pattern)) {
    const rawUrl = match[0].replace(/[),.;:\]}]+$/g, '');
    let parsed;
    try { parsed = new URL(rawUrl); } catch (_) { continue; }
    const segments = parsed.pathname.split('/').filter(Boolean);
    if (segments.length < 2) {
      unsupported.push({ url: rawUrl, reason: 'not_a_repository' });
      continue;
    }
    // GitHub owner/repo is case-insensitive; canonicalize to lowercase so
    // dedup sets, repo-binding guards, and task_repository labels all agree.
    const owner = segments[0].toLowerCase();
    const repoName = segments[1].replace(/\.git$/i, '').toLowerCase();
    if (!owner || !repoName) continue;
    const type = (segments[2] || '').toLowerCase();
    const isIssueItem = /^(issues|pull)$/.test(type)
      && segments.length === 4
      && /^\d+$/.test(segments[3] || '');
    const isIssuesList = type === 'issues' && segments.length === 3;
    const isRepoRoot = segments.length === 2;
    const repo = `${owner}/${repoName}`;
    let kind = null;
    let url = null;
    let number = null;
    if (isIssueItem) {
      kind = type === 'issues' ? 'issue' : 'pr';
      number = Number(segments[3]);
      url = `https://github.com/${repo}/${type}/${number}`;
    } else if (isIssuesList) {
      kind = 'issues-list';
      url = `https://github.com/${repo}/issues`;
    } else if (isRepoRoot) {
      kind = 'repository';
      url = `https://github.com/${repo}`;
    } else {
      unsupported.push({
        url: `https://github.com/${repo}/${segments.slice(2).join('/')}`,
        reason: 'unsupported_path',
      });
      continue;
    }
    if (seen.has(url)) continue;
    seen.add(url);
    refs.push({ url, repo, kind, number });
  }
  return { refs, unsupported };
}

function normalizeGithubRemote(value) {
  const text = String(value || '').trim();
  const match = text.match(/github\.com[/:]([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+?)(?:\.git)?$/i);
  return match ? `${match[1]}/${match[2]}`.toLowerCase() : null;
}

function projectGithubRepository(projectDir) {
  if (!projectDir) return null;
  try {
    const config = fs.readFileSync(path.join(projectDir, '.git', 'config'), 'utf8');
    const origin = config.match(/\[remote\s+"origin"\][\s\S]*?\n\s*url\s*=\s*([^\r\n]+)/i);
    if (origin) return normalizeGithubRemote(origin[1]);
    const anyRemote = config.match(/\n\s*url\s*=\s*([^\r\n]+)/i);
    return anyRemote ? normalizeGithubRemote(anyRemote[1]) : null;
  } catch (_) {
    return null;
  }
}

function uniqueGoalId(projectDir, objective, refs) {
  // loopx enforces GLOBAL uniqueness of goal ids (a global route with the
  // same id but a different repo rejects bootstrap). Union the project
  // registry ids with the global registry ids so a fresh suffix is chosen
  // instead of colliding with a stale global route.
  const existing = new Set();
  for (const dir of [projectDir, null]) {
    const { registry } = readRegistry(dir);
    for (const goal of (registry && registry.goals) || []) {
      const id = goal.goal_id || goal.id;
      if (id) existing.add(id);
    }
  }
  const issueRefs = refs.filter((ref) => ref.kind === 'issue' || ref.kind === 'pr');
  const listRef = refs.find((ref) => ref.kind === 'issues-list');
  // One task = one repository: the id carries BOTH owner and repo name, so
  // two repositories with the same name under different owners can never
  // collide into a suffix sibling goal (legacy ids were owner-less).
  const owner = String((listRef || issueRefs[0] || {}).repo || '').split('/')[0].toLowerCase();
  const repoSlug = (ref) => (owner ? `${owner}-` : '') + String(ref.repo).split('/')[1];
  let base;
  if (listRef) {
    base = `${repoSlug(listRef)}-issues`;
  } else if (issueRefs.length === 1) {
    base = `${repoSlug(issueRefs[0])}-issue-${issueRefs[0].number}`;
  } else if (issueRefs.length > 1) {
    base = `${repoSlug(issueRefs[0])}-issues`;
  } else {
    base = String(objective)
      .toLowerCase()
      .replace(/https?:\/\/\S+/g, ' ')
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 42);
  }
  if (!base) {
    const stamp = new Date().toISOString().replace(/[-:TZ.]/g, '').slice(0, 14);
    base = `task-${stamp}`;
  }
  // v3.2: miniapp-created goals carry the bfx- prefix so the board can tell
  // them apart from goals created by other loopx hosts on this machine.
  base = `bfx-${base}`;
  let goalId = base;
  let suffix = 2;
  while (existing.has(goalId)) goalId = `${base}-${suffix++}`;
  return goalId;
}

function readGoalObjective(projectDir, goal) {
  const stateFile = goal && goal.state_file;
  if (!stateFile) return null;
  const root = projectDir || goal.repo;
  if (!root) return null;
  try {
    const content = fs.readFileSync(path.resolve(root, stateFile), 'utf8');
    const yaml = content.match(/^objective:\s*(.+)$/m);
    if (yaml) {
      const raw = yaml[1].trim();
      if (raw.startsWith('"')) {
        try { return JSON.parse(raw); } catch (_) {}
      }
      return raw.replace(/^['"]|['"]$/g, '');
    }
    const section = content.match(/## Objective\s+([\s\S]*?)(?:\n## |$)/);
    return section ? section[1].trim() : null;
  } catch (_) {
    return null;
  }
}

async function planIssueIntake({ argvPrefix, srcDir, projectDir, url }) {
  const planArgsBase = ['issue-fix', 'workflow-plan', '--url', String(url).trim()];
  if (projectDir) planArgsBase.push('--repo-path', projectDir);
  let result, payload;
  ({ result, payload } = await runJson(argvPrefix, projectDir, [
    ...planArgsBase, '--fetch-metadata', '--fetch-timeout-seconds', '20',
  ], { srcDir, timeoutMs: 90000 }));
  let fetchedMetadata = true;
  const planEmpty = (value) => !value || value.ok === false
    || !(value.ordered_loopx_todo_writeback_preview || []).length;
  if (planEmpty(payload)) {
    fetchedMetadata = false;
    ({ result, payload } = await runJson(argvPrefix, projectDir, planArgsBase, {
      srcDir, timeoutMs: 90000,
    }));
  }
  if (!payload) {
    return { ok: false, error: result.stderr.trim() || 'loopx returned no JSON payload', raw: null };
  }
  const preview = (payload.ordered_loopx_todo_writeback_preview || [])
    .filter((entry) => entry && entry.task_class && entry.text)
    .map((entry) => ({
      order: entry.planner_order ?? null,
      role: entry.role || 'agent',
      priority: entry.priority ?? null,
      taskClass: entry.task_class,
      actionKind: entry.action_kind ?? null,
      text: entry.text,
    }));
  return {
    ok: result.code === 0 && payload.ok !== false,
    fetchedMetadata,
    issueSignal: payload.issue_signal ?? null,
    branchPlan: payload.branch_plan ?? null,
    feasibilityRoutes: payload.feasibility_checkpoint_plan?.routes ?? null,
    todosPreview: preview,
    raw: payload,
  };
}

async function writePlannedTodos({ argvPrefix, srcDir, projectDir, goalId, agentId, intake }) {
  const preview = intake.todosPreview || [];
  if (!preview.length) return { ...intake, ok: false, error: 'workflow-plan produced no writable todos' };
  // Lowercase to match githubReferences' canonical repo labels — a goal must
  // not accumulate two case-variant task_repository identities.
  const repoLabel = (intake.issueSignal?.repo || '').toLowerCase() || null;
  const written = [];
  for (const todo of preview) {
    const args = [
      'todo', 'add', '--goal-id', goalId, '--role', todo.role,
      '--task-class', todo.taskClass, '--text', todo.text,
    ];
    if (todo.actionKind) args.push('--action-kind', todo.actionKind);
    if (repoLabel) args.push('--task-repository', `git:github.com/${repoLabel}`);
    if (todo.role === 'agent' && agentId) args.push('--claimed-by', agentId);
    try {
      const response = await runJson(argvPrefix, projectDir, args, { srcDir });
      const ok = response.result.code === 0 && response.payload?.ok !== false;
      // Plan-level user gates (e.g. "approve this fix plan" with its
      // write/publish scopes) were implicitly approved when the user
      // confirmed the intake sheet — complete them immediately so the task
      // auto-runs instead of parking in a gate that repeats the consent
      // already given.
      if (ok && todo.role === 'user' && response.payload?.todo_id) {
        const completeArgs = [
          'todo', 'complete', '--goal-id', goalId,
          '--todo-id', response.payload.todo_id,
          '--note', '由任务入库确认自动批准',
        ];
        if (todo.taskClass === 'user_gate') completeArgs.push('--decision-outcome', 'approve');
        try {
          await runJson(argvPrefix, projectDir, completeArgs, { srcDir, timeoutMs: 60000 });
        } catch (_) {}
      }
      written.push({
        actionKind: todo.actionKind,
        ok,
        todoId: response.payload?.todo_id ?? null,
        error: ok ? null : (response.payload?.error || response.result.stderr.slice(0, 200) || 'todo add failed'),
      });
    } catch (err) {
      written.push({ actionKind: todo.actionKind, ok: false, todoId: null, error: String(err.message || err) });
    }
  }
  return { ...intake, written, ok: intake.ok && written.every((entry) => entry.ok) };
}

// ── GitHub issue enumeration (batch-fix intake) ───────────
// loopx itself has no repo-wide issue listing (issue-fix workflow-plan takes
// exactly one --url), so the console enumerates open issues via the anonymous
// GitHub REST API. Public repos only; unauthenticated quota is 60 req/hour.
// With a token the request authenticates (Bearer) and POST/PUT become
// available — used by the PR publish flow (fork / push / pull request).
function githubApiRequest(apiPath, {
  method = 'GET', token = null, jsonBody = null, allow404 = false,
} = {}) {
  return new Promise((resolve, reject) => {
    const bodyText = jsonBody === null ? null : JSON.stringify(jsonBody);
    const headers = {
      'User-Agent': 'bitfun-loopx',
      Accept: 'application/vnd.github+json',
      'X-GitHub-Api-Version': '2022-11-28',
    };
    if (token) headers.Authorization = `Bearer ${token}`;
    if (bodyText) headers['Content-Type'] = 'application/json';
    const req = https.request({
      hostname: 'api.github.com',
      path: apiPath,
      method,
      headers,
      timeout: 30000,
    }, (res) => {
      let body = '';
      res.setEncoding('utf8');
      res.on('data', (chunk) => { body += chunk; });
      res.on('end', () => {
        if (allow404 && res.statusCode === 404) { resolve(null); return; }
        if (res.statusCode === 403 || res.statusCode === 429) {
          reject(new Error(res.headers['x-ratelimit-remaining'] === '0'
            ? 'GitHub API rate limit exceeded; retry later'
            : `GitHub API refused the request (HTTP ${res.statusCode})`));
          return;
        }
        if (res.statusCode === 401) {
          reject(new Error('GitHub token is invalid or expired'));
          return;
        }
        if (res.statusCode === 404) {
          reject(new Error(`GitHub repository not found (or private): ${apiPath.split('?')[0]}`));
          return;
        }
        if (res.statusCode < 200 || res.statusCode >= 300) {
          reject(new Error(`GitHub API HTTP ${res.statusCode} for ${method} ${apiPath.split('?')[0]}`));
          return;
        }
        try {
          resolve(JSON.parse(body));
        } catch (err) {
          reject(new Error(`GitHub API returned invalid JSON: ${err.message}`));
        }
      });
    });
    req.on('timeout', () => { req.destroy(new Error('GitHub API request timed out')); });
    req.on('error', reject);
    if (bodyText) req.write(bodyText);
    req.end();
  });
}

function githubApiGet(apiPath) {
  return githubApiRequest(apiPath);
}

// Re-pasting an already-intaken issue must not mint a duplicate todo: check
// the goal's current todos for an open entry containing this issue URL.
async function hasOpenIssueTodo(argvPrefix, srcDir, projectDir, goalId, url) {
  try {
    const { result, payload } = await runJson(argvPrefix, projectDir, ['todo', 'list', '--goal-id', goalId], { srcDir, timeoutMs: 60000 });
    if (result.code !== 0) return false;
    const todos = (payload && payload.todos) || [];
    return todos.some((td) => td.status !== 'done' && String(td.text || td.title || '').includes(url));
  } catch (_) {
    return false;
  }
}

// One-shot git invocation bound to a checkout directory.
function gitRun(projectDir, args, timeoutMs = 60000) {
  return new Promise((resolve, reject) => {
    const child = spawn('git', args, { cwd: projectDir, windowsHide: true });
    let stdout = '';
    let stderr = '';
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', (d) => { stdout += d; });
    child.stderr.on('data', (d) => { stderr += d; });
    const timer = setTimeout(() => {
      try { child.kill(); } catch (_) {}
      reject(new Error(`git ${args[0]} timed out after ${Math.round(timeoutMs / 1000)}s`));
    }, timeoutMs);
    child.on('error', (err) => { clearTimeout(timer); reject(err); });
    child.on('close', (code) => {
      clearTimeout(timer);
      if (code === 0) resolve({ stdout, stderr });
      else reject(new Error(`git ${args[0]} failed (exit ${code}): ${stderr.trim().slice(0, 300)}`));
    });
  });
}

// BitFun itself does not store GitHub tokens — its own review platform
// reuses the local GitHub CLI credentials. Do the same here: when the user
// has run `gh auth login` on this machine, publish can authenticate without
// a pasted PAT.
function ghExeName() {
  return process.platform === 'win32' ? 'gh.exe' : 'gh';
}

function ghExeCandidates() {
  const list = [];
  const localAppData = process.env.LOCALAPPDATA;
  if (localAppData) list.push(path.join(localAppData, 'Programs', 'GitHub CLI', 'gh.exe'));
  if (process.platform === 'darwin') {
    list.push('/opt/homebrew/bin/gh', '/usr/local/bin/gh');
  } else if (process.platform === 'linux') {
    list.push('/usr/local/bin/gh', '/usr/bin/gh');
  }
  // User-local install dir used by the direct-download fallback (no root and
  // no package manager required); scanned on every platform.
  list.push(path.join(consoleHome(), 'bin', ghExeName()));
  list.push('gh');
  return list;
}

// gh is not on PATH right after a fresh winget install (the worker's PATH is
// fixed at spawn time), so resolve the exe explicitly and reuse it everywhere.
async function findGhExe() {
  for (const candidate of ghExeCandidates()) {
    if (candidate === 'gh') {
      try {
        const probe = await spawnLoopx({ argv: ['gh'] }, ['--version'], { timeoutMs: 8000 });
        if (probe.code === 0) return 'gh';
      } catch (_) { /* fall through to absolute paths */ }
    } else if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  return null;
}

function ghCliToken() {
  return new Promise((resolve) => {
    findGhExe().then((gh) => {
      if (!gh) { resolve(null); return; }
      const child = spawn(gh, ['auth', 'token'], { windowsHide: true });
      let stdout = '';
      child.stdout.setEncoding('utf8');
      child.stdout.on('data', (d) => { stdout += d; });
      const timer = setTimeout(() => {
        try { child.kill(); } catch (_) {}
        resolve(null);
      }, 10000);
      child.on('error', () => { clearTimeout(timer); resolve(null); });
      child.on('close', (code) => {
        clearTimeout(timer);
        const token = stdout.trim();
        resolve(code === 0 && token ? token : null);
      });
    });
  });
}

// Windows system proxy (WinINET). gh and winget honor it when network is
// otherwise restricted; surfaced in the login progress so the user knows the
// download is going through their configured proxy.
function readWindowsSystemProxy() {
  return new Promise((resolve) => {
    try {
      const child = spawn('reg', ['query', 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings', '/v', 'ProxyServer'], { windowsHide: true });
      let out = '';
      child.stdout.setEncoding('utf8');
      child.stdout.on('data', (d) => { out += d; });
      child.on('error', () => resolve(null));
      child.on('close', () => {
        const m = out.match(/ProxyServer\s+REG_SZ\s+([^\r\n]+)/i);
        resolve(m ? m[1].trim() : null);
      });
    } catch (_) { resolve(null); }
  });
}

function proxyUrlFrom(value) {
  if (!value) return null;
  // Multi-protocol forms like "http=127.0.0.1:7890;https=127.0.0.1:7890"
  const first = String(value).split(';')[0].replace(/^[a-z]+=/, '').trim();
  if (!first) return null;
  return /^https?:\/\//i.test(first) ? first : `http://${first}`;
}

// POSIX mirror of the Windows system-proxy lookup: curl and gh honor the
// standard https_proxy environment variables when present.
function envProxyUrl() {
  const value = process.env.HTTPS_PROXY || process.env.https_proxy
    || process.env.HTTP_PROXY || process.env.http_proxy;
  return proxyUrlFrom(value);
}

// winget is not guaranteed to exist (App Installer is absent on some
// machines); resolve it by absolute candidates and fall back to the PATH.
function wingetCandidates() {
  const list = [];
  const localAppData = process.env.LOCALAPPDATA;
  if (localAppData) {
    list.push(path.join(localAppData, 'Microsoft', 'WindowsApps', 'winget.exe'));
    list.push(path.join(localAppData, 'Microsoft', 'WinGet', 'Links', 'winget.exe'));
  }
  list.push('winget');
  return list;
}

async function findWinget() {
  for (const candidate of wingetCandidates()) {
    if (candidate === 'winget') {
      try {
        const probe = await spawnLoopx({ argv: ['winget'] }, ['--version'], { timeoutMs: 8000 });
        if (probe.code === 0) return 'winget';
      } catch (_) { /* fall through */ }
    } else if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  return null;
}

function powershellExe() {
  const sys = 'C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe';
  return fs.existsSync(sys) ? sys : 'powershell';
}

// Platform release-asset shape for the direct gh download (used when no
// package manager is available). gh ships windows_amd64.zip, macOS_<arch>.zip
// and linux_<arch>.tar.gz.
function ghReleaseAsset() {
  if (process.platform === 'win32') {
    return { pattern: /windows_amd64\.zip$/i, name: 'windows_amd64.zip', archive: 'zip' };
  }
  if (process.platform === 'darwin') {
    const arch = process.arch === 'arm64' ? 'arm64' : 'amd64';
    return { pattern: new RegExp(`macOS_${arch}\\.zip$`, 'i'), name: `macOS_${arch}.zip`, archive: 'zip' };
  }
  if (process.platform === 'linux') {
    const arch = process.arch === 'arm64' ? 'arm64' : 'amd64';
    return { pattern: new RegExp(`linux_${arch}\\.tar\\.gz$`, 'i'), name: `linux_${arch}.tar.gz`, archive: 'tar.gz' };
  }
  return null;
}

// Direct-download gh installer (no winget/brew/root required): fetches the
// platform release archive from GitHub and places the single gh binary into a
// user-owned directory that findGhExe already scans. Windows downloads through
// PowerShell's Invoke-WebRequest (honors the system proxy); POSIX uses curl,
// which honors https_proxy.
async function installGhFromRelease(emit) {
  const assetInfo = ghReleaseAsset();
  if (!assetInfo) {
    throw new Error('当前平台暂不支持自动安装 GitHub CLI，请手动安装：https://github.com/cli/cli/releases');
  }
  emit('未找到包管理器，改为直接下载 GitHub CLI 安装包…');
  let url = null;
  try {
    const release = await githubApiRequest('/repos/cli/cli/releases/latest');
    const asset = release && Array.isArray(release.assets)
      ? release.assets.find((a) => a && assetInfo.pattern.test(String(a.browser_download_url || '')))
      : null;
    url = asset ? asset.browser_download_url : null;
  } catch (_) { /* fall through to the error below */ }
  if (!url) {
    throw new Error(`无法获取 GitHub CLI 最新版本信息。请手动安装：https://github.com/cli/cli/releases（下载 ${assetInfo.name} 并解压）`);
  }
  const isWindows = process.platform === 'win32';
  const dstDir = isWindows
    ? path.join(process.env.LOCALAPPDATA || os.tmpdir(), 'Programs', 'GitHub CLI')
    : path.join(consoleHome(), 'bin');
  const binName = ghExeName();
  const dst = path.join(dstDir, binName);
  const stamp = Date.now();
  const tmpArchive = path.join(os.tmpdir(), `gh-install-${stamp}${assetInfo.archive === 'zip' ? '.zip' : '.tar.gz'}`);
  const expandDir = path.join(os.tmpdir(), `gh-extract-${stamp}`);
  try {
    emit(`下载 ${url.slice(0, 120)} …`);
    if (isWindows) {
      const ps = `[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri '${url}' -OutFile '${tmpArchive}' -UseBasicParsing`;
      const dl = await spawnLoopx(
        { argv: [powershellExe()] },
        ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', ps],
        { timeoutMs: 300000, onStderrLine: (line) => emit(String(line).slice(0, 140)) },
      );
      if (dl.code !== 0 || !fs.existsSync(tmpArchive)) throw new Error('下载失败');
    } else {
      const dl = await spawnLoopx(
        { argv: ['curl'] },
        ['-fL', '--retry', '3', '-o', tmpArchive, url],
        { timeoutMs: 300000, onStderrLine: (line) => emit(String(line).slice(0, 140)) },
      );
      if (dl.code !== 0 || !fs.existsSync(tmpArchive)) {
        throw new Error('下载失败（需要 curl，或手动安装：https://github.com/cli/cli/releases）');
      }
    }
    emit('下载完成，正在解压…');
    fs.mkdirSync(expandDir, { recursive: true });
    if (isWindows) {
      const ex = await spawnLoopx(
        { argv: [powershellExe()] },
        ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', `Expand-Archive -Path '${tmpArchive}' -DestinationPath '${expandDir}' -Force`],
        { timeoutMs: 120000, onStderrLine: (line) => emit(String(line).slice(0, 140)) },
      );
      if (ex.code !== 0) throw new Error('解压失败');
    } else if (assetInfo.archive === 'zip') {
      const ex = await spawnLoopx(
        { argv: ['unzip'] },
        ['-o', tmpArchive, '-d', expandDir],
        { timeoutMs: 120000, onStderrLine: (line) => emit(String(line).slice(0, 140)) },
      );
      if (ex.code !== 0) throw new Error('解压失败（需要 unzip）');
    } else {
      const ex = await spawnLoopx(
        { argv: ['tar'] },
        ['-xzf', tmpArchive, '-C', expandDir],
        { timeoutMs: 120000, onStderrLine: (line) => emit(String(line).slice(0, 140)) },
      );
      if (ex.code !== 0) throw new Error('解压失败（需要 tar）');
    }
    // Release layout: <root>/gh_<version>_<platform>/bin/gh[.exe]
    let ghBin = null;
    for (const entry of fs.readdirSync(expandDir, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const candidate = path.join(expandDir, entry.name, 'bin', binName);
      if (fs.existsSync(candidate)) { ghBin = candidate; break; }
    }
    if (!ghBin) throw new Error('安装包内未找到 gh 二进制');
    fs.mkdirSync(dstDir, { recursive: true });
    fs.copyFileSync(ghBin, dst);
    if (!isWindows) fs.chmodSync(dst, 0o755);
    emit('GitHub CLI 安装完成');
  } catch (err) {
    throw new Error(`直接安装失败：${String(err.message || err)}。可手动安装：https://github.com/cli/cli/releases`);
  } finally {
    try { fs.rmSync(tmpArchive, { force: true }); } catch (_) {}
    try { fs.rmSync(expandDir, { recursive: true, force: true }); } catch (_) {}
  }
}

// Try the platform package manager for gh: winget on Windows, Homebrew on
// macOS. Returns null when gh was installed or no package manager exists;
// returns a user-facing error string when an install was attempted and failed
// (never silently fall back to the direct download after a visible failure).
async function installGhViaPackageManager(emit, envOverlay) {
  if (process.platform === 'win32') {
    const winget = await findWinget();
    if (!winget) return null;
    emit(`通过 winget（${winget}）安装…`);
    try {
      const install = await spawnLoopx(
        { argv: [winget], env: envOverlay },
        ['install', '--id', 'GitHub.cli', '-e', '--accept-source-agreements', '--accept-package-agreements'],
        { timeoutMs: 600000, onStderrLine: (line) => emit(String(line).slice(0, 140)) },
      );
      if (install.code !== 0) {
        throw new Error(String(install.stderr || install.stdout || '').trim().slice(-200) || `winget exit ${install.code}`);
      }
    } catch (err) {
      return `安装 GitHub CLI 失败：${String(err.message || err)}。可手动安装后再试：https://github.com/cli/cli/releases`;
    }
    return null;
  }
  if (process.platform === 'darwin') {
    const brew = await findBrew();
    if (!brew) return null;
    emit(`通过 Homebrew（${brew}）安装…`);
    try {
      const install = await spawnLoopx(
        { argv: [brew], env: envOverlay },
        ['install', 'gh'],
        { timeoutMs: 600000, onStderrLine: (line) => emit(String(line).slice(0, 140)) },
      );
      if (install.code !== 0) {
        throw new Error(String(install.stderr || install.stdout || '').trim().slice(-200) || `brew exit ${install.code}`);
      }
    } catch (err) {
      return `安装 GitHub CLI 失败：${String(err.message || err)}。可手动安装后再试：https://github.com/cli/cli/releases`;
    }
    return null;
  }
  // Linux has no universal package manager; the direct release download
  // covers it without root privileges.
  return null;
}

async function findBrew() {
  for (const candidate of ['/opt/homebrew/bin/brew', '/usr/local/bin/brew']) {
    if (!fs.existsSync(candidate)) continue;
    try {
      const probe = await spawnLoopx({ argv: [candidate] }, ['--version'], { timeoutMs: 8000 });
      if (probe.code === 0) return candidate;
    } catch (_) { /* try the next candidate */ }
  }
  return null;
}

// gh's web login flow needs an interactive terminal to display the one-time
// code: Windows opens a fresh console via `cmd start`, macOS opens a Terminal
// window via osascript, Linux tries the common terminal emulators in order.
// Resolves null once a launcher has been started (best-effort) or a
// user-facing guidance string when no interactive terminal could be opened.
function launchGhAuthLogin(gh, envOverlay) {
  const authArgs = ['auth', 'login', '--hostname', 'github.com', '--git-protocol', 'https', '--web'];
  const spawnDetached = (exe, args, opts = {}) => new Promise((resolve) => {
    let done = false;
    const finish = (value) => { if (!done) { done = true; resolve(value); } };
    try {
      const child = spawn(exe, args, {
        detached: true,
        stdio: 'ignore',
        env: envOverlay ? { ...process.env, ...envOverlay } : process.env,
        ...opts,
      });
      child.on('spawn', () => finish(null));
      child.on('error', (err) => finish(String(err && err.message || err)));
      // No early error within the window: assume the launcher took over.
      setTimeout(() => finish(null), 1500);
    } catch (err) {
      finish(String(err && err.message || err));
    }
  });
  const guidance = () => '无法自动打开登录终端。请在自己的终端执行 `gh auth login` 后回到控制台重试。';
  if (process.platform === 'win32') {
    return spawnDetached('cmd', ['/c', 'start', '', gh, ...authArgs], { windowsHide: false }).then(
      (err) => (err ? `无法启动登录窗口：${err}。${guidance()}` : null),
    );
  }
  if (process.platform === 'darwin') {
    // JSON.stringify yields a double-quoted, escaped token; joining them and
    // wrapping the result in another JSON.stringify produces a valid
    // AppleScript string literal for `do script`.
    const cmdLine = [gh, ...authArgs].map((s) => JSON.stringify(String(s))).join(' ');
    const script = `tell application "Terminal" to do script ${JSON.stringify(cmdLine)}`;
    return spawnDetached('osascript', ['-e', script]).then(
      (err) => (err ? `无法打开终端窗口：${err}。${guidance()}` : null),
    );
  }
  // Linux terminal emulators take the program and its args directly; no shell
  // quoting is involved.
  const terminals = [
    ['x-terminal-emulator', ['-e', gh, ...authArgs]],
    ['gnome-terminal', ['--', gh, ...authArgs]],
    ['konsole', ['-e', gh, ...authArgs]],
    ['xterm', ['-e', gh, ...authArgs]],
  ];
  return (async () => {
    for (const [exe, args] of terminals) {
      const err = await spawnDetached(exe, args);
      if (err === null) return null;
    }
    return guidance();
  })();
}

// Issue bodies frequently carry the real problem in screenshots. Detect image
// references (markdown images, <img> tags, GitHub attachment URLs) so the UI
// can be conservative with text-only models.
function bodyHasImages(body) {
  const s = String(body || '');
  if (!s) return false;
  if (/!\[[^\]]*\]\([^)]*\)/i.test(s)) return true;
  if (/<img\b/i.test(s)) return true;
  if (/user[-_](images|attachments)\.githubusercontent\.com|github\.com\/user-attachments\/assets/i.test(s)) return true;
  return false;
}

async function fetchOpenIssues(repo) {
  const issues = [];
  // /repos/…/issues interleaves PRs (every PR is an issue) — filter them out,
  // otherwise a busy repo's fix loop would keep "fixing" its own PRs.
  for (let page = 1; page <= 3; page += 1) {
    const batch = await githubApiGet(`/repos/${repo}/issues?state=open&per_page=100&page=${page}`);
    if (!Array.isArray(batch)) break;
    for (const item of batch) {
      if (!item || item.pull_request || !item.number) continue;
      issues.push({
        number: item.number,
        title: item.title || `#${item.number}`,
        url: `https://github.com/${repo}/issues/${item.number}`,
        labels: (item.labels || []).map((l) => (typeof l === 'string' ? l : l && l.name)).filter(Boolean),
        comments: item.comments ?? 0,
        updatedAt: item.updated_at || null,
        hasImages: bodyHasImages(item.body),
      });
    }
    if (batch.length < 100) return { issues, truncated: false };
  }
  // Page 3 came back full — more issues/PRs likely exist beyond the cap.
  return { issues, truncated: true };
}

module.exports = {
  async 'loopx.detect'({ argvPrefix = null, srcDir = null } = {}) {
    dbgWorker('detect:start', `argvPrefix=${JSON.stringify(argvPrefix)} srcDir=${srcDir || ''}`);
    return detectLoopx(argvPrefix, srcDir);
  },

  // One-click bootstrap: pip-install loopx from source. Progress lines stream
  // through installLoopx:progress events. Absolute python.exe candidates come
  // first (the worker PATH may be restricted); pip-on-PATH is the fallback.
  async 'loopx.installLoopx'({} = {}) {
    const emit = (line) => global.rpcEmit('installLoopx:progress', { line });
    const target = 'git+https://github.com/huangruiteng/loopx.git';
    const attempts = [];
    for (const pythonExe of pythonCandidates()) {
      attempts.push({ argv: [pythonExe, '-m', 'pip', 'install', target] });
    }
    attempts.push({ argv: ['pip', 'install', target] });
    attempts.push({ argv: ['python', '-m', 'pip', 'install', target] });
    if (process.platform !== 'win32') {
      attempts.push({ argv: ['python3', '-m', 'pip', 'install', target] });
    }
    let lastError = null;
    for (const prefix of attempts) {
      emit(`$ ${prefix.argv.join(' ')}`);
      try {
        const { code, stderr } = await spawnLoopx(prefix, [], {
          timeoutMs: 600000,
          onStderrLine: (line) => emit(String(line).slice(0, 140)),
        });
        if (code === 0) {
          emit('install complete');
          cachedPrefix = null;
          const detected = await detectLoopx(null, null);
          return { ok: true, found: detected.found, version: detected.version || null };
        }
        lastError = stderr.slice(-300) || `exit ${code}`;
      } catch (err) {
        lastError = String(err.message || err);
      }
      emit(`failed: ${lastError.slice(0, 140)}`);
    }
    return { ok: false, error: lastError || 'pip install failed' };
  },

  // Prerequisite probe for the universal acquisition path. loopx itself has no
  // runtime dependencies, but running it from source needs a Python >= 3.11
  // interpreter and fetching the source needs git. Reported per-item so the UI
  // can name exactly what is missing (never a silent failure).
  async 'loopx.checkPrereqs'({} = {}) {
    const python = await probePython();
    const git = await probeGit();
    return { ready: !!(python.ok && git.found), python, git };
  },

  // One-click vendor: clone (or re-pin) the loopx source checkout into
  // ~/.bitfun/bitfun-loopx/vendor/loopx at the known-good version
  // LOOPX_VENDOR_REF and run it straight from there via
  // `python -m loopx.cli` + PYTHONPATH. No pip, no global install. Progress
  // streams through vendorLoopx:progress events.
  async 'loopx.ensureVendor'({} = {}) {
    const emit = (line) => global.rpcEmit('vendorLoopx:progress', { line });
    const prereqs = await checkPrereqs();
    if (!prereqs.ready) {
      return { ok: false, stage: 'prereqs', prereqs, error: prereqErrorMessage(prereqs) };
    }
    const dir = vendorLoopxDir();
    const gitEnv = { GIT_TERMINAL_PROMPT: '0' };
    try {
      if (!fs.existsSync(path.join(dir, 'loopx', 'cli.py'))) {
        emit(`$ git clone --depth 1 --branch ${LOOPX_VENDOR_REF} ${LOOPX_REPO_URL} "${dir}"`);
        const clone = await spawnLoopx(
          { argv: ['git'], env: gitEnv },
          ['clone', '--depth', '1', '--branch', LOOPX_VENDOR_REF, LOOPX_REPO_URL, dir],
          { timeoutMs: 300000, onStderrLine: (line) => emit(String(line).slice(0, 140)) },
        );
        if (clone.code !== 0) {
          throw new Error(clone.stderr.trim().slice(-200) || `git clone failed (exit ${clone.code})`);
        }
      } else {
        // Existing checkout: re-pin to LOOPX_VENDOR_REF best-effort (offline
        // keeps the current copy; the pin heals on a later successful run).
        emit(`$ git -C "${dir}" fetch --depth 1 origin tag ${LOOPX_VENDOR_REF}`);
        try {
          await spawnLoopx(
            { argv: ['git'], env: gitEnv },
            ['-C', dir, 'fetch', '--depth', '1', 'origin', 'tag', LOOPX_VENDOR_REF],
            { timeoutMs: 120000, onStderrLine: (line) => emit(String(line).slice(0, 140)) },
          );
          await spawnLoopx(
            { argv: ['git'], env: gitEnv },
            ['-C', dir, '-c', 'advice.detachedHead=false', 'checkout', '--force', LOOPX_VENDOR_REF],
            { timeoutMs: 60000, onStderrLine: (line) => emit(String(line).slice(0, 140)) },
          );
        } catch (_) { /* stale checkout stays usable */ }
      }
      const detected = await detectLoopx(null, dir);
      if (detected.found) {
        cachedPrefix = detected.argvPrefix;
        return {
          ok: true,
          found: true,
          version: detected.version,
          srcDir: dir,
          argvPrefix: detected.argvPrefix,
          prereqs,
        };
      }
      const detail = (detected.probes || [])
        .map((p) => `${(p.argvPrefix || []).join(' ')} → ${p.ok ? p.version : p.error || 'failed'}`)
        .join('\n');
      return {
        ok: false,
        stage: 'detect',
        prereqs,
        error: `loopx source fetched but not runnable (Python >= 3.11 required):\n${detail}`,
      };
    } catch (err) {
      return { ok: false, stage: 'clone', prereqs, error: String(err.message || err) };
    }
  },

  async 'loopx.doctor'({ argvPrefix = null, projectDir = null } = {}) {
    const { result, payload } = await runJson(argvPrefix, projectDir, ['doctor']);
    return { ok: result.code === 0, payload, stderr: result.stderr };
  },

  // v3.2: adopt a goal created by another loopx host on this machine so the
  // board starts monitoring it. Registers the agent on the goal (idempotent);
  // execution turns still need a known project directory, which the UI binds
  // separately.
  async 'loopx.adoptGoal'({
    argvPrefix = null, srcDir = null, projectDir = null, goalId, agentId,
  } = {}) {
    if (!goalId) throw new Error('loopx.adoptGoal: goalId is required');
    if (!agentId) {
      return { ok: true, goalId, registered: false, error: null };
    }
    dbgWorker('adoptGoal:start', `goalId=${goalId} agentId=${agentId}`);
    const { result, payload } = await runJson(argvPrefix, projectDir, [
      'register-agent', '--goal-id', goalId, '--agent-id', agentId, '--execute',
    ], { srcDir, timeoutMs: 60000 });
    const ok = result.code === 0 && payload?.ok !== false;
    dbgWorker('adoptGoal:done', `ok=${ok}`);
    return {
      ok,
      goalId,
      registered: ok,
      error: ok ? null : (payload?.error || result.stderr.slice(0, 300) || 'agent registration failed'),
    };
  },

  // Mid-task human intervention: a free-text message from the user is written
  // as a user-lane todo bound to the goal's agent lane (task-class
  // user_action — NOT user_gate: this is a message, not a blocking decision).
  // loopx delivers it as the "post-response continuation" for that lane, so
  // the agent reads it on its next turn.
  async 'loopx.guideGoal'({
    argvPrefix = null, srcDir = null, projectDir = null,
    goalId, agentId = null, text,
  } = {}) {
    if (!goalId) throw new Error('loopx.guideGoal: goalId is required');
    const message = String(text || '').trim();
    if (!message) throw new Error('loopx.guideGoal: text is required');
    if (message.length > 2000) throw new Error('loopx.guideGoal: text is too long (max 2000 characters)');
    dbgWorker('guideGoal:start', `goalId=${goalId} agentId=${agentId || ''} text=${message.slice(0, 80)}`);
    const args = [
      'todo', 'add', '--goal-id', goalId,
      '--role', 'user', '--task-class', 'user_action', '--text', message,
    ];
    if (agentId) args.push('--bound-agent', agentId);
    const { result, payload } = await runJson(argvPrefix, projectDir, args, { srcDir, timeoutMs: 60000 });
    const ok = result.code === 0 && payload?.ok !== false;
    dbgWorker('guideGoal:done', `ok=${ok}`);
    return {
      ok,
      goalId,
      todoId: payload?.todo_id ?? null,
      error: ok ? null : (payload?.error || result.stderr.slice(0, 300) || 'todo add failed'),
    };
  },

  // "Paste an issue URL" glue: preview via `issue-fix workflow-plan
  // --fetch-metadata`, then (execute=true) materialize the ordered todo
  // preview into a goal with plain `loopx todo add` calls. Uses only
  // shipped loopx CLI surface — no loopx-side changes.
  async 'loopx.issueIntake'({
    argvPrefix = null,
    srcDir = null,
    projectDir = null,
    url,
    goalId = null,
    execute = false,
  } = {}) {
    if (!url || !/^https:\/\/github\.com\//.test(String(url).trim())) {
      throw new Error('loopx.issueIntake: url must be a public https://github.com/ issue or PR link');
    }
    const intake = await planIssueIntake({ argvPrefix, srcDir, projectDir, url });
    if (!execute) return intake;
    if (!goalId) throw new Error('loopx.issueIntake: goalId is required to write todos');
    return writePlannedTodos({ argvPrefix, srcDir, projectDir, goalId, agentId: null, intake });
  },

  // Probe the local GitHub CLI credential (`gh auth token`). The UI uses it
  // before asking the user to paste a PAT: a machine that already ran
  // `gh auth login` needs no manual token.
  async 'loopx.githubGhToken'() {
    const token = await ghCliToken();
    return { ok: Boolean(token), token: token || null };
  },

  // One-click full reset: back up (rename) and wipe every loopx data
  // location this console touches — the global registry/runtime, per-checkout
  // registries and goal states, and the console's clone cache. Nothing is
  // unlinked: everything moves into a timestamped backup directory so the
  // operation stays recoverable.
  async 'loopx.resetAll'({ projectDirs = null } = {}) {
    const stamp = new Date().toISOString().replace(/[-:TZ.]/g, '').slice(0, 14);
    const backupRoot = path.join(consoleHome(), `cleared-${stamp}`);
    fs.mkdirSync(backupRoot, { recursive: true });
    const moved = [];
    const renameAway = (src, name) => {
      if (!fs.existsSync(src)) return;
      const dst = path.join(backupRoot, name);
      try {
        fs.renameSync(src, dst);
        moved.push(dst);
      } catch (err) {
        throw new Error(`cannot move ${src} into the backup: ${err.message}`);
      }
    };
    dbgWorker('resetAll:start', `backup=${backupRoot}`);
    renameAway(path.join(os.homedir(), '.codex', 'loopx'), 'loopx-global');
    renameAway(cloneCacheRoot(), 'repos-clone-cache');
    for (const dir of (Array.isArray(projectDirs) ? projectDirs : [])) {
      if (!dir) continue;
      renameAway(path.join(dir, '.loopx'), `checkout-${Buffer.from(dir).toString('base64').slice(0, 24)}-loopx`);
      renameAway(path.join(dir, '.codex', 'goals'), `checkout-${Buffer.from(dir).toString('base64').slice(0, 24)}-goals`);
    }
    dbgWorker('resetAll:done', `moved=${moved.length}`);
    return { ok: true, backupDir: backupRoot, moved };
  },

  // Product-level task intake. This adapts natural-language goals, repository
  // URLs, and one or more Issue/PR URLs onto LoopX's public CLI without
  // changing LoopX itself.
  //
  // Fast read-only classification for the composer: parses the input, expands
  // an owner/repo/issues list URL into concrete open issues (GitHub REST,
  // anonymous), and runs the repo-binding checks — so the UI can show a
  // precise confirmation sheet before anything is written.
  async 'loopx.resolveIntake'({ projectDir = null, projectDirs = null, objective } = {}) {
    const text = String(objective || '').trim();
    if (!text) throw new Error('loopx.resolveIntake: objective is required');
    dbgWorker('resolveIntake:start', `text=${text.slice(0, 120)}`);
    const { refs, unsupported } = githubReferences(text);
    dbgWorker('resolveIntake:refs', `refs=${JSON.stringify(refs.map((r) => r.kind + ':' + r.url))} unsupported=${unsupported.length}`);
    if (unsupported.length) {
      return {
        ok: false,
        code: 'unsupported_github_path',
        url: unsupported[0].url,
        error: `Unsupported GitHub link: ${unsupported[0].url}. Paste an issue, a pull request, the repository home, or its issues list.`,
      };
    }
    if (!refs.length) {
      return {
        ok: false,
        code: 'unsupported_input',
        error: 'Paste a GitHub issue, pull request, repository, or issues-list link.',
      };
    }
    const issueRefs = refs.filter((ref) => ref.kind === 'issue' || ref.kind === 'pr');
    const listRefs = refs.filter((ref) => ref.kind === 'issues-list');
    const requestedRepos = [...new Set(refs.map((ref) => ref.repo))];
    const projectRepo = projectGithubRepository(projectDir);
    if (requestedRepos.length > 1) {
      return { ok: false, code: 'multiple_repositories', requestedRepos, projectRepo };
    }
    if (requestedRepos.length === 1 && projectRepo && requestedRepos[0] !== projectRepo) {
      return {
        ok: false,
        code: 'repository_mismatch',
        requestedRepo: requestedRepos[0],
        projectRepo,
      };
    }
    // A GitHub-targeted task against a project dir we cannot identify (no
    // .git/config or non-GitHub remote) would silently run fixes in the wrong
    // tree. Surface it as its own blocking code.
    if (requestedRepos.length === 1 && !projectRepo && projectDir) {
      return {
        ok: false,
        code: 'repository_unverified',
        requestedRepo: requestedRepos[0],
        projectRepo: null,
      };
    }
    // No global checkout, or the caller deliberately bypassed it: before
    // offering a fresh clone, look for an existing checkout of this exact
    // repository among the console's recorded project directories AND the
    // stable clone cache — a fresh MiniApp import (empty config) then reuses
    // the cached checkout immediately, without even a GitHub lookup.
    let reuseDir = null;
    if (requestedRepos.length === 1 && !projectDir) {
      const searchDirs = Array.isArray(projectDirs)
        ? projectDirs.filter((dir) => typeof dir === 'string' && dir)
        : [];
      try {
        const root = cloneCacheRoot();
        if (fs.existsSync(root)) {
          for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
            if (entry.isDirectory()) searchDirs.push(path.join(root, entry.name));
          }
        }
      } catch (_) {}
      for (const dir of searchDirs) {
        if (projectGithubRepository(dir) === requestedRepos[0]) {
          reuseDir = dir;
          dbgWorker('resolveIntake:reuse', `${requestedRepos[0]} -> ${dir}`);
          break;
        }
      }
    }
    // No local checkout at all: direction C — offer auto-clone into the
    // MiniApp's own data directory. Verify the repository exists first so a
    // typo fails before the user confirms.
    let autoClone = false;
    if (requestedRepos.length === 1 && !projectDir && !reuseDir) {
      let exists = true;
      try {
        dbgWorker('resolveIntake:repoExists:start', requestedRepos[0]);
        exists = await repoExistsOnGithub(requestedRepos[0]);
        dbgWorker('resolveIntake:repoExists:done', `${requestedRepos[0]} exists=${exists}`);
      } catch (err) {
        dbgWorker('resolveIntake:repoExists:error', `${err.message}`);
        return {
          ok: false,
          code: 'repository_lookup_failed',
          requestedRepo: requestedRepos[0],
          error: `Could not verify ${requestedRepos[0]} on GitHub: ${err.message}`,
        };
      }
      if (!exists) {
        return {
          ok: false,
          code: 'repository_not_found',
          requestedRepo: requestedRepos[0],
          error: `GitHub repository not found: ${requestedRepos[0]}`,
        };
      }
      autoClone = true;
    }
    let issues = issueRefs.filter((ref) => ref.kind === 'issue').map((ref) => ({
      number: ref.number, title: `#${ref.number}`, url: ref.url, fromList: false,
      hasImages: false,
    }));
    // Single-issue intake: fetch the body so the sheet can warn about images
    // that a text-only model cannot see (bounded to a handful of links to
    // stay inside the anonymous rate limit).
    if (issues.length > 0 && issues.length <= 5) {
      await Promise.all(issues.map(async (issue) => {
        try {
          const detail = await githubApiGet(`/repos/${refs[0].repo}/issues/${issue.number}`);
          if (detail && typeof detail.body === 'string') issue.hasImages = bodyHasImages(detail.body);
        } catch (_) { /* conservative: keep false */ }
      }));
    }
    let kind = issueRefs.length ? (issueRefs.length > 1 ? 'issues' : 'issue') : 'repository';
    let truncated = false;
    // Both an explicit issues-list URL and a bare repository URL mean "the
    // repo's open issues" — one link, one behavior, the sheet re-selects.
    const repoRefs = refs.filter((ref) => ref.kind === 'repository');
    const expandRepo = listRefs.length ? listRefs[0].repo
      : (repoRefs.length && !issueRefs.length ? repoRefs[0].repo : null);
    if (expandRepo) {
      kind = 'issues-list';
      dbgWorker('resolveIntake:fetchIssues:start', expandRepo);
      const fetched = await fetchOpenIssues(expandRepo);
      dbgWorker('resolveIntake:fetchIssues:done', `count=${fetched.issues.length} truncated=${fetched.truncated}`);
      truncated = fetched.truncated;
      const seen = new Set(issues.map((issue) => issue.url));
      for (const issue of fetched.issues) {
        if (!seen.has(issue.url)) issues.push({ ...issue, fromList: true });
      }
    }
    return {
      ok: true,
      kind,
      repo: (refs[0] && refs[0].repo) || projectRepo || null,
      projectRepo,
      reuseDir,
      issues,
      truncated,
      prCount: issueRefs.filter((ref) => ref.kind === 'pr').length,
      autoClone,
    };
  },

  // List a goal's todos (zero-write projection). Used to surface open
  // user-lane gates on the board.
  // Delete a task entirely: archive its runtime directory via loopx
  // (--allow-registered --execute) and remove the registry entry, keeping a
  // backup of the registry file before rewriting it.
  async 'loopx.deleteGoal'({ argvPrefix = null, srcDir = null, projectDir = null, goalId } = {}) {
    if (!goalId) throw new Error('loopx.deleteGoal: goalId is required');
    dbgWorker('deleteGoal:start', `goalId=${goalId} projectDir=${projectDir || '(global)'}`);
    // Archive tolerantly: the runtime may already be archived by an earlier
    // attempt (or loopx's own archival) — that still counts as archived for
    // our purposes, so the registry surgery below always gets its chance.
    let archived = false;
    let archiveError = null;
    for (const dir of projectDir ? [projectDir, null] : [null]) {
      try {
        const archive = await runJson(argvPrefix, dir, [
          'archive-runtime', '--goal-id', goalId, '--allow-registered', '--execute',
        ], { srcDir, timeoutMs: 120000 });
        if (archive.result.code === 0 && archive.payload?.ok !== false) {
          archived = true;
          archiveError = null;
          break;
        }
        archiveError = archive.payload?.error || archive.result.stderr.trim() || 'archive-runtime failed';
        // A missing source dir means it is already archived — good enough.
        if (/does not exist|not found|already archived/i.test(archiveError)) {
          archived = true;
          archiveError = null;
          break;
        }
      } catch (err) {
        archiveError = String(err.message || err);
      }
    }
    if (!archived) {
      return {
        ok: false, goalId, archived: false, registryRemoved: false,
        error: archiveError || 'archive-runtime failed',
      };
    }
    // Registry surgery: drop the goal entry from the project AND the global
    // registry (loopx keeps a global route for every goal), with timestamped
    // backups of both files.
    const registryPaths = [...new Set([
      projectDir ? path.join(projectDir, '.loopx', 'registry.json') : resolveRegistryPath(null),
      resolveRegistryPath(null),
    ])];
    let registryRemoved = false;
    for (const registryPath of registryPaths) {
      try {
        const raw = fs.readFileSync(registryPath, 'utf8');
        const registry = JSON.parse(raw);
        if (Array.isArray(registry.goals)) {
          const before = registry.goals.length;
          registry.goals = registry.goals.filter((goal) => (goal.goal_id || goal.id) !== goalId);
          if (registry.goals.length < before) {
            const stamp = new Date().toISOString().replace(/[-:TZ.]/g, '').slice(0, 14);
            fs.copyFileSync(registryPath, `${registryPath}.del-bak-${stamp}`);
            registry.updated_at = new Date().toISOString();
            fs.writeFileSync(registryPath, `${JSON.stringify(registry, null, 2)}\n`);
            registryRemoved = true;
          }
        }
      } catch (err) {
        dbgWorker('deleteGoal:registryError', `${err.message}`);
      }
    }
    // Belt and braces: a leftover runtime directory can be re-registered by a
    // later loopx sync and resurrect the goal — move it aside under a backup
    // name instead of leaving it in place.
    try {
      const goalsRoot = path.join(os.homedir(), '.codex', 'loopx', 'goals');
      const runtimeDir = path.join(goalsRoot, goalId);
      if (fs.existsSync(runtimeDir)) {
        const stamp = new Date().toISOString().replace(/[-:TZ.]/g, '').slice(0, 14);
        fs.renameSync(runtimeDir, `${runtimeDir}.del-bak-${stamp}`);
        dbgWorker('deleteGoal:runtimeMoved', `${runtimeDir}.del-bak-${stamp}`);
      }
    } catch (err) {
      dbgWorker('deleteGoal:runtimeMoveError', `${err.message}`);
    }
    // Archived runtimes live under archived-goals/<goalId>-<stamp> and are
    // re-listed by listGoals — deleting an ALREADY-archived goal must move
    // those aside too, or the goal resurrects on the next refresh and the UI
    // dropdown never updates. Rename (never delete) per the backup discipline.
    try {
      const archiveRoot = path.join(os.homedir(), '.codex', 'loopx', 'archived-goals');
      if (fs.existsSync(archiveRoot)) {
        for (const entry of fs.readdirSync(archiveRoot, { withFileTypes: true })) {
          if (!entry.isDirectory()) continue;
          const m = entry.name.match(/^(bfx-.*)-(\d{8}T\d{6}Z)$/);
          if (!m || m[1] !== goalId) continue;
          const src = path.join(archiveRoot, entry.name);
          const stamp = new Date().toISOString().replace(/[-:TZ.]/g, '').slice(0, 14);
          fs.renameSync(src, path.join(archiveRoot, `${entry.name}.del-bak-${stamp}`));
          dbgWorker('deleteGoal:archiveMoved', src);
        }
      }
    } catch (err) {
      dbgWorker('deleteGoal:archiveMoveError', `${err.message}`);
    }
    dbgWorker('deleteGoal:done', `archived=${archived} registryRemoved=${registryRemoved}`);
    return {
      ok: true,
      goalId,
      archived,
      registryRemoved,
      archivePath: null,
      warning: registryRemoved ? null : 'runtime archived, but the registry entry could not be removed',
    };
  },

  // Restore an archived console goal: move the newest archived runtime back
  // under <runtime-root>/goals and re-add the registry entry (reconstructed
  // from the newest registry .del-bak backup that still carries it, with a
  // minimal fallback). The task comes back paused; 继续 resumes as usual.
  async 'loopx.restoreGoal'({ projectDir = null, goalId, archiveDir = null } = {}) {
    if (!goalId) throw new Error('loopx.restoreGoal: goalId is required');
    dbgWorker('restoreGoal:start', `goalId=${goalId}`);
    const loopxRoot = path.join(os.homedir(), '.codex', 'loopx');
    // 1. Locate the newest archive dir for this goal.
    const archiveRoot = path.join(loopxRoot, 'archived-goals');
    let source = archiveDir && fs.existsSync(archiveDir) ? archiveDir : null;
    if (!source && fs.existsSync(archiveRoot)) {
      let best = null;
      let bestStamp = '';
      for (const entry of fs.readdirSync(archiveRoot, { withFileTypes: true })) {
        if (!entry.isDirectory()) continue;
        const m = entry.name.match(/^(bfx-.*)-(\d{8}T\d{6}Z)$/);
        if (!m || m[1] !== goalId) continue;
        if (m[2] > bestStamp) { bestStamp = m[2]; best = path.join(archiveRoot, entry.name); }
      }
      source = best;
    }
    if (!source) return { ok: false, goalId, error: 'archived goal directory not found' };
    // 2. Move the runtime back into the goals root.
    const goalsRoot = path.join(loopxRoot, 'goals');
    fs.mkdirSync(goalsRoot, { recursive: true });
    const target = path.join(goalsRoot, goalId);
    if (fs.existsSync(target)) return { ok: false, goalId, error: 'goal runtime dir already exists' };
    fs.renameSync(source, target);
    // 3. Rebuild the registry entry from the newest del-bak that carries it.
    let entry = null;
    const bakFiles = fs.existsSync(loopxRoot)
      ? fs.readdirSync(loopxRoot).filter((f) => f.startsWith('registry.global.json.del-bak-')).sort().reverse()
      : [];
    for (const f of bakFiles) {
      try {
        const reg = JSON.parse(fs.readFileSync(path.join(loopxRoot, f), 'utf8'));
        const hit = (reg.goals || []).find((g) => (g.goal_id || g.id) === goalId);
        if (hit) { entry = hit; break; }
      } catch (_) {}
    }
    if (!entry) {
      entry = {
        id: goalId, domain: 'project-goal-control-plane', status: 'active',
        role: 'controller', parent_goal_id: null,
        coordination: { registered_agents: ['bitfun-agent'] },
      };
    }
    // 4. Re-add the entry to the global registry and the repo registry.
    const repoDir = (entry && entry.repo) || projectDir || null;
    const targets = [path.join(loopxRoot, 'registry.global.json')];
    if (repoDir) targets.push(path.join(repoDir, '.loopx', 'registry.json'));
    let restored = false;
    for (const rp of targets) {
      try {
        const raw = fs.readFileSync(rp, 'utf8');
        const reg = JSON.parse(raw);
        if (!Array.isArray(reg.goals)) reg.goals = [];
        if (!reg.goals.some((g) => (g.goal_id || g.id) === goalId)) {
          reg.goals.push(entry);
          reg.updated_at = new Date().toISOString();
          fs.writeFileSync(rp, `${JSON.stringify(reg, null, 2)}\n`);
          restored = true;
        }
      } catch (err) {
        dbgWorker('restoreGoal:registryError', `${err.message}`);
      }
    }
    dbgWorker('restoreGoal:done', `restored=${restored}`);
    return { ok: true, goalId, restored, runtimeDir: target, repoDir: repoDir || null };
  },

  async 'loopx.listTodos'({ argvPrefix = null, projectDir = null, goalId, role = null, status = null } = {}) {
    if (!goalId) throw new Error('loopx.listTodos: goalId is required');
    const { result, payload } = await runJson(argvPrefix, projectDir, ['todo', 'list', '--goal-id', goalId]);
    let todos = (payload && payload.todos) || [];
    if (role) todos = todos.filter((todo) => todo.role === role);
    if (status) todos = todos.filter((todo) => todo.status === status);
    return { ok: result.code === 0, todos, error: result.code === 0 ? null : result.stderr.slice(0, 300) };
  },

  // Per-goal issue tracker: batch intake writes one agent todo per issue
  // ("[P1] Fix GitHub issue #N: <title> (<url>)", action_kind=fix_issue), so
  // the goal's issue list + per-issue status is a projection over those todos.
  // Returns the structured board the card strip renders: url/number/title and
  // the todo status (open / blocked / deferred / done).
  async 'loopx.goalIssues'({ argvPrefix = null, projectDir = null, goalId } = {}) {
    if (!goalId) throw new Error('loopx.goalIssues: goalId is required');
    const { result, payload } = await runJson(argvPrefix, projectDir, ['todo', 'list', '--goal-id', goalId]);
    const todos = (payload && payload.todos) || [];
    const issues = [];
    for (const td of todos) {
      if (td.role && td.role !== 'agent') continue;
      const text = String(td.text || td.title || '');
      const labeled = text.match(/fix github issue #(\d+)\s*[:：]?\s*(.*?)\s*\((https?:\/\/github\.com\/[^\s)]+)\)/i);
      if (!labeled && td.action_kind !== 'fix_issue') continue;
      let url = null;
      let number = null;
      let title = '';
      if (labeled) {
        number = Number(labeled[1]);
        title = labeled[2] || '';
        url = labeled[3];
      } else {
        const bare = text.match(/https?:\/\/github\.com\/[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+\/(?:issues|pull)\/(\d+)/i);
        if (!bare) continue;
        url = bare[0];
        number = Number(bare[1]);
      }
      const status = td.status || 'open';
      issues.push({
        url,
        number,
        title,
        status,
        done: status === 'done',
        todoId: td.todo_id ?? null,
      });
    }
    const done = issues.filter((issue) => issue.done).length;
    return {
      ok: result.code === 0,
      issues,
      total: issues.length,
      done,
      open: issues.length - done,
      error: result.code === 0 ? null : result.stderr.slice(0, 300),
    };
  },

  // Commit subjects for a branch — feeds the PR "解决方案" section and the
  // publish-time cause/solution analysis prompt.
  async 'loopx.gitLog'({ projectDir = null, branch = null, limit = 15 } = {}) {
    if (!projectDir) throw new Error('loopx.gitLog: projectDir is required');
    const args = ['log', `-${Math.min(Math.max(Number(limit) || 15, 1), 30)}`, '--no-merges', '--pretty=format:%s'];
    if (branch) args.push(branch);
    const log = await gitRun(projectDir, args, 30000);
    return {
      ok: true,
      subjects: String(log.stdout || '').split(/\r?\n/).map((s) => s.trim()).filter(Boolean),
    };
  },

  // Changed files of a branch (merge-base..branch) — feeds the PR "涉及文件"
  // section and makes the generated solution concrete about WHAT was touched.
  async 'loopx.gitDiff'({ projectDir = null, branch = null } = {}) {
    if (!projectDir) throw new Error('loopx.gitDiff: projectDir is required');
    const target = branch || 'HEAD';
    let range = null;
    for (const ref of ['master', 'main', 'origin/master', 'origin/main']) {
      try {
        const mb = await gitRun(projectDir, ['merge-base', ref, target], 20000);
        const sha = String(mb.stdout || '').trim();
        if (sha) { range = `${sha}..${target}`; break; }
      } catch (_) { /* try the next base ref */ }
    }
    const effective = range || target;
    const names = await gitRun(projectDir, ['diff', '--name-only', effective], 30000);
    const stat = await gitRun(projectDir, ['diff', '--shortstat', effective], 30000);
    return {
      ok: true,
      range,
      files: String(names.stdout || '').split(/\r?\n/).map((s) => s.trim()).filter(Boolean),
      stat: String(stat.stdout || '').trim() || null,
    };
  },

  // Complete (approve) one todo by id — the console's gate-approval action.
  // user_gate todos hard-require --decision-outcome (approve/reject/cancel);
  // the CLI rejects the flag on other task classes, so pass it conditionally.
  async 'loopx.completeTodo'({
    argvPrefix = null, srcDir = null, projectDir = null,
    goalId, todoId, note = null, decisionOutcome = null,
  } = {}) {
    if (!goalId || !todoId) throw new Error('loopx.completeTodo: goalId and todoId are required');
    const args = ['todo', 'complete', '--goal-id', goalId, '--todo-id', todoId];
    if (decisionOutcome) args.push('--decision-outcome', decisionOutcome);
    if (note) args.push('--note', note);
    const { result, payload } = await runJson(argvPrefix, projectDir, args, { srcDir });
    const ok = result.code === 0 && payload?.ok !== false;
    return { ok, payload, error: ok ? null : (payload?.error || result.stderr.slice(0, 300) || 'todo complete failed') };
  },

  // Validates a GitHub token and returns its login — the settings dialog
  // checks the token at configuration time instead of at publish time.
  async 'loopx.githubUser'({ token = null } = {}) {
    if (!token) throw new Error('loopx.githubUser: token is required');
    const user = await githubApiRequest('/user', { token });
    return { ok: true, login: user && user.login ? user.login : null };
  },

  // One-click GitHub CLI login: install gh when missing (winget on Windows,
  // Homebrew on macOS, direct release download everywhere else), then launch
  // `gh auth login --web` in its own terminal window — gh prints a one-time
  // code there and opens the browser. The worker polls `gh auth status` until
  // the browser flow completes. Progress streams through ghLogin:progress
  // events.
  async 'loopx.ghLogin'({} = {}) {
    const emit = (line) => global.rpcEmit('ghLogin:progress', { line });
    const proxyUrl = process.platform === 'win32'
      ? proxyUrlFrom(await readWindowsSystemProxy())
      : envProxyUrl();
    const envOverlay = proxyUrl ? { HTTPS_PROXY: proxyUrl, HTTP_PROXY: proxyUrl } : undefined;
    let gh = await findGhExe();
    if (!gh) {
      emit('GitHub CLI 未安装，正在安装…');
      if (proxyUrl) emit(`检测到代理：${proxyUrl}（安装将经此代理下载）`);
      const pkgError = await installGhViaPackageManager(emit, envOverlay);
      gh = await findGhExe();
      if (!gh && pkgError) return { ok: false, error: pkgError };
      if (!gh) {
        // No package manager available (or a platform without one): download
        // the release archive directly into a user-owned directory.
        try {
          await installGhFromRelease(emit);
        } catch (err) {
          return { ok: false, error: String(err.message || err) };
        }
        gh = await findGhExe();
      }
      if (!gh) return { ok: false, error: '安装完成但未找到 gh，请重开控制台后重试' };
    }
    emit(`已找到 ${gh}。启动浏览器登录（弹出窗口会显示一次性代码，浏览器确认后自动完成）…`);
    const launchError = await launchGhAuthLogin(gh, envOverlay);
    if (launchError) return { ok: false, error: launchError };
    // The login poll can take minutes: emit a heartbeat line every 30s so the
    // dialog always shows which step is in progress and that the worker is
    // still alive (the RPC call itself may run for several minutes).
    emit('登录终端已打开。请在浏览器中完成 GitHub 授权，控制台会自动检测结果（最多等待 8 分钟）…');
    const deadline = Date.now() + 8 * 60000;
    let lastWaitEmitAt = 0;
    while (Date.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, 4000));
      if (Date.now() - lastWaitEmitAt >= 30000) {
        lastWaitEmitAt = Date.now();
        const waitedMin = Math.max(0, Math.round((8 * 60000 - (deadline - Date.now())) / 60000));
        emit(`仍在等待浏览器登录…（已等待约 ${waitedMin} 分钟 / 最多 8 分钟）`);
      }
      try {
        const status = await spawnLoopx({ argv: [gh], env: envOverlay }, ['auth', 'status'], { timeoutMs: 15000 });
        if (status.code === 0) {
          let login = null;
          try {
            const user = await spawnLoopx({ argv: [gh], env: envOverlay }, ['api', 'user', '-q', '.login'], { timeoutMs: 20000 });
            if (user.code === 0) login = String(user.stdout || '').trim() || null;
          } catch (_) {}
          emit(`登录完成${login ? `：${login}` : ''}`);
          return { ok: true, login };
        }
      } catch (_) { /* still waiting */ }
    }
    return {
      ok: false,
      error: '等待浏览器登录超时（8 分钟）。请在弹出的窗口确认登录后重试，或改用方式二粘贴 Token。',
    };
  },

  // Publish the current fix branch as a PR through the user's fork: ensure
  // the fork exists (create it when missing), push the branch, then open a
  // pull request against the upstream default branch. When issueUrl is given,
  // the PR is composed properly: title binds the issue, body links the issue
  // with a one-line description, an analysis of the cause and the solution
  // (generated by the UI and passed as `analysis`), and the branch's commit
  // subjects — all tagged [bitfun-loopx] for countability.
  async 'loopx.publishPr'({
    projectDir = null, goalId = null, token = null, title = '', body = '', branch: requestedBranch = null,
    issueUrl = null, analysis = null,
  } = {}) {
    if (!projectDir) throw new Error('loopx.publishPr: projectDir is required');
    if (!token) {
      // No configured PAT: fall back to the local GitHub CLI credential so a
      // machine that ran `gh auth login` publishes without a pasted token.
      token = await ghCliToken();
      if (!token) throw new Error('loopx.publishPr: token is required (configure one in the console, or run gh auth login)');
    }
    dbgWorker('publishPr:start', `goalId=${goalId || ''} branch=${requestedBranch || ''} issueUrl=${issueUrl || ''}`);
    let branch = (await gitRun(projectDir, ['rev-parse', '--abbrev-ref', 'HEAD'])).stdout.trim();
    if (!branch || branch === 'HEAD') throw new Error('publishPr: the checkout is in detached HEAD state');
    // Prefer the branch named by the gate item; keep HEAD when unknown.
    if (requestedBranch && requestedBranch !== branch) {
      try {
        await gitRun(projectDir, ['rev-parse', '--verify', `refs/heads/${requestedBranch}`], 20000);
        branch = requestedBranch;
      } catch (_) {
        dbgWorker('publishPr:branchFallback', `${requestedBranch} -> HEAD`);
      }
    }
    const origin = (await gitRun(projectDir, ['remote', 'get-url', 'origin'])).stdout.trim();
    const repoMatch = origin.match(/github\.com[:/]([^/\s]+)\/([^/\s]+?)(?:\.git)?$/i);
    if (!repoMatch) throw new Error(`publishPr: cannot parse repository from origin "${origin}"`);
    const owner = repoMatch[1];
    const repo = repoMatch[2].replace(/\.git$/i, '');
    const user = await githubApiRequest('/user', { token });
    const login = String((user && user.login) || '').trim();
    if (!login) throw new Error('publishPr: GitHub token did not resolve a login');

    // ── PR content composition: bind the issue, describe the problem and the
    // changes. The UI's title/body stay as the fallback for non-issue goals.
    let finalTitle = title;
    let finalBody = body;
    let composedSections = null; // sections with a __COMMITS__ slot
    let composedSolution = null;
    const issueMatch = String(issueUrl || '').match(/\/(?:issues|pull)\/(\d+)/);
    if (issueMatch) {
      const issueNumber = Number(issueMatch[1]);
      let issueInfo = null;
      try {
        issueInfo = await githubApiRequest(`/repos/${owner}/${repo}/issues/${issueNumber}`, { token });
      } catch (_) { /* compose without the live issue body */ }
      const issueTitle = (issueInfo && issueInfo.title) || '';
      const titleSuffix = issueTitle
        ? `Fix #${issueNumber}: ${String(issueTitle).slice(0, 80)}`
        : `Fix #${issueNumber}`;
      finalTitle = `${PR_TITLE_PREFIX}${titleSuffix}`;
      composedSections = [
        `Fixes #${issueNumber}`,
        `## 相关 Issue\n\nhttps://github.com/${owner}/${repo}/issues/${issueNumber}${issueTitle ? ` — ${issueTitle}` : ''}`,
      ];
      const cause = analysis && typeof analysis.cause === 'string' && analysis.cause.trim() ? analysis.cause.trim() : null;
      composedSolution = analysis && typeof analysis.solution === 'string' && analysis.solution.trim() ? analysis.solution.trim() : null;
      if (cause) composedSections.push(`## 问题原因\n\n${cause}`);
      composedSections.push('__COMMITS__', PR_BODY_MARKER);
    }

    // Fork: reuse an existing fork; create and wait when absent.
    let fork = await githubApiRequest(`/repos/${login}/${repo}`, { token, allow404: true });
    if (!fork) {
      dbgWorker('publishPr:forking', `${owner}/${repo} -> ${login}/${repo}`);
      await githubApiRequest(`/repos/${owner}/${repo}/forks`, { token, method: 'POST', jsonBody: {} });
      for (let i = 0; i < 30; i += 1) {
        await new Promise((resolve) => setTimeout(resolve, 4000));
        fork = await githubApiRequest(`/repos/${login}/${repo}`, { token, allow404: true });
        if (fork) break;
      }
      if (!fork) throw new Error('publishPr: the fork did not become ready within 2 minutes');
    }

    // Push the fix branch to the user's fork. The token rides the push URL
    // for this single command; nothing is written to git config.
    const pushUrl = `https://x-access-token:${token}@github.com/${login}/${repo}.git`;
    dbgWorker('publishPr:push', `${login}/${repo}@${branch}`);
    try {
      await gitRun(projectDir, ['push', pushUrl, `HEAD:refs/heads/${branch}`], 300000);
    } catch (err) {
      throw new Error(String(err.message || err).split(token).join('***'));
    }

    // Open the PR against the upstream default branch.
    const upstream = await githubApiRequest(`/repos/${owner}/${repo}`, { token });
    const base = (upstream && upstream.default_branch) || 'main';
    if (composedSections) {
      // Only the branch's OWN commits (merge-base..HEAD): a raw `git log -15
      // HEAD` drags in unrelated upstream history (docs/ci commits inherited
      // from main) and buries the actual fix under noise.
      let subjects = [];
      let changedFiles = [];
      let statLine = '';
      try {
        let range = null;
        for (const ref of [base, `origin/${base}`, 'main', 'master']) {
          try {
            const mb = await gitRun(projectDir, ['merge-base', ref, 'HEAD'], 20000);
            const sha = String(mb.stdout || '').trim();
            if (sha) { range = `${sha}..HEAD`; break; }
          } catch (_) { /* try the next base ref */ }
        }
        const log = await gitRun(projectDir, ['log', '-15', '--no-merges', '--pretty=format:%s', range || 'HEAD'], 30000);
        subjects = String(log.stdout || '').split(/\r?\n/).map((s) => s.trim()).filter(Boolean);
        if (range) {
          const names = await gitRun(projectDir, ['diff', '--name-only', range], 30000);
          changedFiles = String(names.stdout || '').split(/\r?\n/).map((s) => s.trim()).filter(Boolean);
          const stat = await gitRun(projectDir, ['diff', '--shortstat', range], 30000);
          statLine = String(stat.stdout || '').trim();
        }
      } catch (_) { /* omit the commit/file list */ }
      const solutionLine = composedSolution ? `${composedSolution}\n` : '';
      const commitLines = subjects.map((s) => `- ${s}`).join('\n');
      const slot = composedSections.indexOf('__COMMITS__');
      const parts = [];
      if (solutionLine || commitLines) {
        parts.push(`## 解决方案\n\n${solutionLine}${commitLines}`.trim());
      }
      if (changedFiles.length) {
        parts.push(`## 涉及文件${statLine ? `（${statLine}）` : ''}\n\n${changedFiles.map((f) => `- ${f}`).join('\n')}`);
      }
      composedSections[slot] = parts.join('\n\n');
      finalBody = composedSections.filter(Boolean).join('\n\n');
    }
    let pr;
    try {
      pr = await githubApiRequest(`/repos/${owner}/${repo}/pulls`, {
        token, method: 'POST',
        jsonBody: { title: finalTitle, head: `${login}:${branch}`, base, body: finalBody },
      });
    } catch (err) {
      // A PR for this branch may already exist (publish retry): reuse it.
      const existing = await githubApiRequest(
        `/repos/${owner}/${repo}/pulls?state=open&head=${encodeURIComponent(`${login}:${branch}`)}`,
        { token },
      );
      const found = Array.isArray(existing) && existing[0];
      if (!found) throw err;
      pr = found;
    }
    const prUrl = pr && pr.html_url ? pr.html_url : '';
    if (!prUrl) throw new Error('publishPr: GitHub did not return a PR url');
    dbgWorker('publishPr:done', `pr=${prUrl}`);
    return {
      ok: true,
      prUrl,
      prNumber: pr.number ?? null,
      branch,
      forkUrl: (fork && fork.html_url) || `https://github.com/${login}/${repo}`,
    };
  },

  // Creates (mode 'new') or steers (mode 'guide') a goal. Returns immediately
  // and reports through taskIntake:progress / taskIntake:done events — a batch
  // of issues takes minutes and must not queue behind the RPC mutex.
  async 'loopx.taskIntake'({
    argvPrefix = null,
    srcDir = null,
    projectDir = null,
    objective,
    agentId,
    mode = 'new',
    goalId = null,
    autoClone = false,
    issues = null, // [{url, number, title}] — pre-confirmed selection from the UI
  } = {}) {
    const text = String(objective || '').trim();
    if (!projectDir && !autoClone) throw new Error('loopx.taskIntake: a local project directory is required (or enable autoClone)');
    dbgWorker('taskIntake:start', `mode=${mode} autoClone=${autoClone} projectDir=${projectDir || '(none)'} text=${text.slice(0, 80)}`);
    if (!text) throw new Error('loopx.taskIntake: objective is required');
    if (text.length > 4000) throw new Error('loopx.taskIntake: objective is too long (max 4000 characters)');
    if (!agentId) throw new Error('loopx.taskIntake: agentId is required');
    if (mode === 'guide' && !goalId) throw new Error('loopx.taskIntake: mode "guide" requires goalId');

    const { refs, unsupported } = githubReferences(text);
    if (unsupported.length) {
      return {
        ok: false,
        code: 'unsupported_github_path',
        url: unsupported[0].url,
        error: `Unsupported GitHub link: ${unsupported[0].url}. Paste an issue, a pull request, the repository home, or its issues list.`,
      };
    }
    if (!refs.length) {
      return {
        ok: false,
        code: 'unsupported_input',
        error: 'Paste a GitHub issue, pull request, repository, or issues-list link.',
      };
    }
    const listRefs = refs.filter((ref) => ref.kind === 'issues-list');
    const requestedRepos = [...new Set(refs.map((ref) => ref.repo))];
    const projectRepo = projectGithubRepository(projectDir);
    if (requestedRepos.length > 1) {
      return {
        ok: false,
        code: 'multiple_repositories',
        error: 'One LoopX goal must target a single local repository. Split links from different repositories into separate tasks.',
        requestedRepos,
      };
    }
    if (requestedRepos.length === 1 && projectRepo && requestedRepos[0] !== projectRepo) {
      return {
        ok: false,
        code: 'repository_mismatch',
        error: `The selected checkout is ${projectRepo}, but the task targets ${requestedRepos[0]}.`,
        requestedRepo: requestedRepos[0],
        projectRepo,
      };
    }
    if (requestedRepos.length === 1 && !projectRepo && projectDir) {
      return {
        ok: false,
        code: 'repository_unverified',
        error: `The selected directory is not a recognizable checkout of ${requestedRepos[0]} (no GitHub remote found).`,
        requestedRepo: requestedRepos[0],
        projectRepo: null,
      };
    }
    if (requestedRepos.length === 1 && !projectDir && !autoClone) {
      return {
        ok: false,
        code: 'project_required',
        error: `No local project directory is selected for ${requestedRepos[0]}; enable auto-clone or select a checkout.`,
        requestedRepo: requestedRepos[0],
      };
    }

    const repoLabel = requestedRepos[0] || projectRepo || null;
    let issueList = Array.isArray(issues) && issues.length
      ? issues.filter((issue) => issue && issue.url)
      : refs.filter((ref) => ref.kind === 'issue' || ref.kind === 'pr')
        .map((ref) => ({ url: ref.url, number: ref.number, title: `#${ref.number}` }));

    const targetGoalId = mode === 'guide' ? goalId : uniqueGoalId(projectDir, text, refs);
    const intakeKind = listRefs.length ? 'issues-list'
      : (issueList.length > 1 ? 'issues' : (issueList.length === 1 ? 'issue' : (refs.length ? 'repository' : 'goal')));
    const emit = (stage, extra = {}) => {
      global.rpcEmit('taskIntake:progress', { goalId: targetGoalId, mode, stage, ...extra });
    };

    (async () => {
      const written = [];
      const issueResults = [];
      const skippedDuplicates = [];
      let failure = null;
      // Which stage failed matters: after bootstrap+register succeed the goal
      // EXISTS, and reporting 'bootstrap_failed' would invite a retry that
      // mints a duplicate goal. Track creation separately from later stages.
      let goalCreated = mode === 'guide'; // guide targets an existing goal
      let failedStage = 'bootstrap';
      let workingDir = projectDir;
      try {
        if (!workingDir && autoClone && requestedRepos.length === 1) {
          emit('clone', { detail: 'start' });
          failedStage = 'clone';
          dbgWorker('taskIntake:clone:start', requestedRepos[0]);
          workingDir = await cloneRepository(requestedRepos[0], (extra) => emit('clone', extra));
          dbgWorker('taskIntake:clone:done', workingDir);
          ensureCommitTrailerHook(workingDir);
        }
        // issues-list / repository URL reaching intake without a UI-confirmed
        // selection (standalone callers): expand it here.
        const expandRepo = listRefs.length ? listRefs[0].repo
          : (!issueList.length && refs.some((ref) => ref.kind === 'repository') ? repoLabel : null);
        if (expandRepo && !issueList.length) {
          emit('expand');
          failedStage = 'expand';
          const fetched = await fetchOpenIssues(expandRepo);
          issueList = fetched.issues.map((issue) => ({ url: issue.url, number: issue.number, title: issue.title }));
        }

        if (mode === 'new') {
          emit('bootstrap');
          failedStage = 'bootstrap';
          const bootstrap = await runJson(argvPrefix, workingDir, [
            'bootstrap', '--project', workingDir, '--goal-id', targetGoalId,
            '--objective', text,
            '--adapter-kind', 'read_only_project_map_v0',
            '--adapter-status', 'connected-read-only',
            '--no-onboarding-scan', '--codex-app-heartbeat', 'ask',
            // The intake confirmation sheet is the user's consent to fix this
            // repository: pre-grant the "write" scope so the agent can edit
            // code without a mid-run approval gate (publish/PR stays gated).
            '--write-scope', 'write',
          ], { srcDir, timeoutMs: 90000 });
          if (bootstrap.result.code !== 0 || bootstrap.payload?.ok === false) {
            throw new Error(bootstrap.payload?.error || bootstrap.result.stderr.trim() || 'loopx bootstrap failed');
          }
        }

        // Registration runs for BOTH modes: todo add --claimed-by rejects
        // agents not registered on the target goal, and in guide mode nothing
        // guarantees the chosen agent is registered there. It is idempotent.
        emit('register');
        failedStage = 'register';
        const registration = await runJson(argvPrefix, workingDir, [
          'register-agent', '--goal-id', targetGoalId, '--agent-id', agentId, '--execute',
        ], { srcDir, timeoutMs: 60000 });
        if (registration.result.code !== 0 || registration.payload?.ok === false) {
          throw new Error(registration.payload?.error || registration.result.stderr.trim() || 'agent registration failed');
        }
        goalCreated = true;

        if (issueList.length === 1) {
          // Single issue: the full workflow-plan path yields ordered,
          // classed todos (branch plan, validation, PR readiness). A
          // re-pasted issue skips the write entirely.
          if (await hasOpenIssueTodo(argvPrefix, srcDir, workingDir, targetGoalId, issueList[0].url)) {
            skippedDuplicates.push(issueList[0]);
            issueResults.push({ url: issueList[0].url, ok: true, skippedDuplicate: true, written: [] });
          } else {
            emit('plan', { current: 1, total: 1, detail: issueList[0].url });
            failedStage = 'plan';
            const intake = await planIssueIntake({ argvPrefix, srcDir, projectDir: workingDir, url: issueList[0].url });
            const result = intake.ok
              ? await writePlannedTodos({ argvPrefix, srcDir, projectDir: workingDir, goalId: targetGoalId, agentId, intake })
              : intake;
            issueResults.push({ url: issueList[0].url, ...result });
            written.push(...(result.written || []));
          }
        } else if (issueList.length > 1) {
          // Batch: one intake todo per issue. Per-issue workflow-plan happens
          // inside the agent's own turns — planning N issues up front would
          // take minutes and burn the anonymous GitHub quota.
          failedStage = 'todos';
          for (let i = 0; i < issueList.length; i += 1) {
            const issue = issueList[i];
            // Per-issue dedup: an open todo for this URL already exists —
            // skip instead of writing a duplicate.
            if (await hasOpenIssueTodo(argvPrefix, srcDir, workingDir, targetGoalId, issue.url)) {
              skippedDuplicates.push(issue);
              written.push({ ok: true, skippedDuplicate: true, actionKind: 'fix_issue', url: issue.url });
              continue;
            }
            emit('todos', { current: i + 1, total: issueList.length, detail: issue.url });
            const label = issue.title && issue.title !== `#${issue.number}`
              ? `Fix GitHub issue #${issue.number}: ${issue.title}`
              : `Fix GitHub issue #${issue.number}`;
            const args = [
              'todo', 'add', '--goal-id', targetGoalId, '--role', 'agent',
              '--task-class', 'advancement_task', '--action-kind', 'fix_issue',
              '--claimed-by', agentId, '--text', `[P1] ${label} (${issue.url})`,
            ];
            if (repoLabel) args.push('--task-repository', `git:github.com/${repoLabel}`);
            try {
              const response = await runJson(argvPrefix, workingDir, args, { srcDir, timeoutMs: 60000 });
              const ok = response.result.code === 0 && response.payload?.ok !== false;
              written.push({
                ok,
                todoId: response.payload?.todo_id ?? null,
                actionKind: 'fix_issue',
                url: issue.url,
                error: ok ? null : (response.payload?.error || response.result.stderr.slice(0, 200) || 'todo add failed'),
              });
            } catch (err) {
              written.push({ ok: false, todoId: null, actionKind: 'fix_issue', url: issue.url, error: String(err.message || err) });
            }
          }
        } else {
          emit('todos', { current: 1, total: 1 });
          failedStage = 'todos';
          const args = [
            'todo', 'add', '--goal-id', targetGoalId, '--role', 'agent',
            '--task-class', 'advancement_task', '--action-kind', 'deliver_user_goal',
            '--claimed-by', agentId, '--text', `[P0] ${text}`,
          ];
          if (repoLabel) args.push('--task-repository', `git:github.com/${repoLabel}`);
          const todo = await runJson(argvPrefix, workingDir, args, { srcDir, timeoutMs: 60000 });
          const ok = todo.result.code === 0 && todo.payload?.ok !== false;
          written.push({
            ok,
            todoId: todo.payload?.todo_id ?? null,
            actionKind: 'deliver_user_goal',
            error: ok ? null : (todo.payload?.error || todo.result.stderr.trim() || 'todo add failed'),
          });
        }

        emit('refresh');
        failedStage = 'refresh';
        // Best-effort: the goal and todos already exist; a refresh hiccup
        // must not be reported as a failed creation.
        try {
          await runJson(argvPrefix, workingDir, [
            'refresh-state', '--goal-id', targetGoalId, '--project', workingDir,
          ], { srcDir, timeoutMs: 60000 });
        } catch (_) {}
      } catch (err) {
        failure = String(err.message || err);
      }
      const ok = !failure && written.length > 0 && written.every((entry) => entry.ok)
        && issueResults.every((entry) => entry.ok);
      const okWritten = written.filter((entry) => entry.ok).length;
      global.rpcEmit('taskIntake:done', {
        ok,
        code: ok ? 'created'
          : (failure ? `${failedStage}_failed` : (okWritten > 0 ? 'todos_partial' : 'todo_write_failed')),
        created: goalCreated,
        mode,
        goalId: targetGoalId,
        objective: text,
        intakeKind,
        issueCount: issueList.length,
        skippedDuplicates: skippedDuplicates.length,
        writtenOk: okWritten,
        repository: repoLabel,
        projectDir: workingDir,
        written,
        issueResults,
        error: failure || (ok ? null : (written.find((entry) => !entry.ok)?.error
          || issueResults.find((entry) => !entry.ok)?.error
          || 'task intake did not produce runnable todos')),
      });
    })();
    return { started: true, goalId: targetGoalId, mode, intakeKind, issueCount: issueList.length };
  },

  async 'loopx.listGoals'({ argvPrefix = null, projectDir = null, projectDirs = null } = {}) {
    const t0 = Date.now();
    dbgWorker('listGoals:start', `projectDir=${projectDir || '(none)'} projectDirs=${Array.isArray(projectDirs) ? projectDirs.length : 0}`);
    // Direction C: goals may live in several registries — the user's global
    // registry, one per cloned/selected project directory, and (since the
    // clone cache is stable across MiniApp re-imports) any repo in the cache
    // that carries its own .loopx registry. Query each and merge by goalId.
    const cacheRepos = [];
    try {
      const root = cloneCacheRoot();
      if (fs.existsSync(root)) {
        for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
          if (!entry.isDirectory()) continue;
          if (fs.existsSync(path.join(root, entry.name, '.loopx', 'registry.json'))) {
            cacheRepos.push(path.join(root, entry.name));
          }
        }
      }
    } catch (_) {}
    const dirs = [projectDir, ...(Array.isArray(projectDirs) ? projectDirs : []), ...cacheRepos]
      .filter((dir) => typeof dir === 'string' && dir);
    const uniqueDirs = [...new Set(dirs)];
    const registryPaths = uniqueDirs
      .map((dir) => path.join(dir, '.loopx', 'registry.json'))
      .filter((p) => fs.existsSync(p));
    const targets = registryPaths.length ? registryPaths : [null];

    const agentsByGoal = {};
    const objectivesByGoal = {};
    const dirByGoal = {};
    for (const target of targets) {
      // target is <dir>/.loopx/registry.json — strip both segments to get the
      // project directory itself.
      const dir = target ? path.dirname(path.dirname(target)) : null;
      const { registry } = readRegistry(dir);
      for (const goal of (registry && registry.goals) || []) {
        const goalId = goal.goal_id || goal.id;
        if (!goalId) continue;
        const coordination = goal.coordination || {};
        agentsByGoal[goalId] = (coordination.registered_agents || [])
          .map((a) => (typeof a === 'string' ? a : a.agent_id || a.id))
          .filter(Boolean);
        objectivesByGoal[goalId] = readGoalObjective(dir, goal);
        if (dir) dirByGoal[goalId] = dir;
      }
    }

    let lastOk = true;
    const groups = {};
    for (const target of targets) {
      const dir = target ? path.dirname(path.dirname(target)) : null;
      // runJson injects --registry <dir>/.loopx/registry.json itself; adding
      // one here would double the flag and make loopx reject the command.
      const args = ['quota', 'status', '--scan-root', quotaScanRoot()];
      const { result, payload } = await runJson(argvPrefix, dir, args);
      if (result.code !== 0) {
        lastOk = false;
        continue;
      }
      const source = (payload && payload.groups) || payload || {};
      if (source && typeof source === 'object') {
        for (const [state, entries] of Object.entries(source)) {
          if (!Array.isArray(entries)) continue;
          if (!groups[state]) groups[state] = [];
          groups[state].push(...entries);
        }
      }
    }
    const goals = [];
    const seen = new Set();
    const pushGoal = (goalId, state, extra = {}) => {
      if (!goalId || seen.has(goalId)) return;
      seen.add(goalId);
      goals.push({
        goalId,
        state: state || null,
        agents: agentsByGoal[goalId] || [],
        objective: objectivesByGoal[goalId] || null,
        projectDir: dirByGoal[goalId] || null,
        ...extra,
      });
    };
    // quota status groups goals by state; shapes vary by schema version, so
    // walk any {state: [goal…]} mapping and any flat goals list defensively.
    for (const [state, entries] of Object.entries(groups)) {
      for (const entry of entries) {
        if (typeof entry === 'string') pushGoal(entry, state);
        else if (entry && typeof entry === 'object') {
          pushGoal(entry.goal_id || entry.id, entry.state || state, {
            waitingOn: entry.waiting_on ?? null,
            status: entry.status ?? null,
          });
        }
      }
    }
    for (const goalId of Object.keys(agentsByGoal)) pushGoal(goalId, null);
    // Archived console goals must never silently vanish from the board: a
    // task whose runtime loopx moved into archived-goals (e.g. a delete the
    // user did not mean to keep) is surfaced with state=archived so the UI
    // can offer 恢复. Only the NEWEST archive per goal id is reported.
    const activeIds = new Set(goals.map((g) => g.goalId));
    const archiveRoot = path.join(os.homedir(), '.codex', 'loopx', 'archived-goals');
    try {
      if (fs.existsSync(archiveRoot)) {
        const newest = new Map(); // goalId -> { stamp, name }
        for (const entry of fs.readdirSync(archiveRoot, { withFileTypes: true })) {
          if (!entry.isDirectory()) continue;
          const m = entry.name.match(/^(bfx-.*)-(\d{8}T\d{6}Z)$/);
          if (!m) continue;
          const goalId = m[1];
          if (activeIds.has(goalId)) continue;
          if (!newest.has(goalId) || m[2] > newest.get(goalId).stamp) {
            newest.set(goalId, { stamp: m[2], name: entry.name });
          }
        }
        for (const [goalId, info] of newest) {
          goals.push({
            goalId,
            state: 'archived',
            agents: [],
            objective: null,
            projectDir: null,
            archived: true,
            archiveDir: path.join(archiveRoot, info.name),
            archiveName: info.name,
          });
        }
      }
    } catch (_) {}
    const registryPath = targets.length === 1
      ? (targets[0] || resolveRegistryPath(null))
      : (registryPaths[0] || null);
    dbgWorker('listGoals:done', `ms=${Date.now() - t0} goals=${goals.length} lastOk=${lastOk}`);
    return { ok: lastOk, registryPath, goals, raw: { groups } };
  },

  async 'loopx.status'({ argvPrefix = null, projectDir = null, goalId = null, agentId = null } = {}) {
    const args = ['status'];
    if (goalId) args.push('--goal-id', goalId);
    if (agentId) args.push('--agent-id', agentId);
    const { result, payload } = await runJson(argvPrefix, projectDir, args);
    return { ok: result.code === 0, payload, stderr: result.stderr };
  },

  async 'loopx.shouldRun'({ argvPrefix = null, projectDir = null, goalId, agentId } = {}) {
    if (!goalId) throw new Error('loopx.shouldRun: goalId is required');
    const t0 = Date.now();
    dbgWorker('shouldRun:start', `goalId=${goalId} projectDir=${projectDir || '(none)'}`);
    const args = [
      'quota', 'should-run',
      '--goal-id', goalId,
      '--runtime-profile', 'outer_controller',
      '--include-scheduler-detail',
      '--scan-root', quotaScanRoot(),
    ];
    if (agentId) args.push('--agent-id', agentId);
    const { result, payload } = await runJson(argvPrefix, projectDir, args);
    dbgWorker('shouldRun:done', `ms=${Date.now() - t0} code=${result.code} hasPayload=${Boolean(payload)}`);
    if (!payload) {
      return { ok: false, error: result.stderr.trim() || 'loopx returned no JSON payload', raw: null };
    }
    return {
      ok: result.code === 0,
      shouldRun: payload.should_run ?? null,
      decision: payload.decision ?? null,
      state: payload.state ?? null,
      effectiveAction: payload.effective_action ?? null,
      reason: payload.reason ?? null,
      recommendedAction: payload.recommended_action ?? null,
      waitingOn: payload.waiting_on ?? null,
      scheduler: normalizeScheduler(payload.scheduler_hint),
      raw: payload,
    };
  },

  // Builds the complete prompt for one execution turn: loopx's own
  // heartbeat-prompt task body (state machine contract, todo discipline,
  // capability scopes) prefixed with the repo/registry binding preamble.
  // The UI feeds this to the HOST's agent (app.agent.run) — BitFun itself is
  // the execution engine; no external CLI host or user configuration.
  async 'loopx.turnPrompt'({
    argvPrefix = null, srcDir = null, projectDir = null, goalId, agentId,
  } = {}) {
    if (!goalId || !agentId) throw new Error('loopx.turnPrompt: goalId and agentId are required');
    if (!projectDir) throw new Error('loopx.turnPrompt: projectDir is required');
    dbgWorker('turnPrompt:start', `goalId=${goalId}`);
    // Re-assert the commit marker hook before every turn (cheap + idempotent):
    // the clone may have been re-fetched or its hooks wiped between turns.
    ensureCommitTrailerHook(projectDir);
    const { result, payload } = await runJson(argvPrefix, projectDir, [
      'heartbeat-prompt', '--goal-id', goalId, '--agent-id', agentId,
      '--runtime-profile', 'outer_controller', '--compact',
    ], { srcDir, timeoutMs: 180000 });
    dbgWorker('turnPrompt:done', `code=${result.code} hasBody=${Boolean(payload && payload.task_body)}`);
    const body = payload && typeof payload.task_body === 'string' ? payload.task_body : null;
    if (result.code !== 0 || payload?.ok === false || !body) {
      return {
        ok: false,
        error: payload?.error || result.stderr.trim() || 'heartbeat-prompt produced no task body',
      };
    }
    // loopx's prompt vocabulary is codex-lineage ("codex" = its label for the
    // agent execution lane; in outer_controller mode that lane IS the BitFun
    // host agent). Strip the one codex-specific instruction so the BitFun
    // agent never reads a reference to a "Codex session" it does not have —
    // keep the intent (do not re-ask granted permissions) in host terms.
    const sanitized = body.replace(
      /\bDo not ask for permissions when the current Codex session is already trusted\./g,
      'Do not re-ask for permissions BitFun has already granted in this session.',
    );
    return { ok: true, prompt: turnPreamble({ projectDir, goalId, agentId }) + sanitized };
  },
};
