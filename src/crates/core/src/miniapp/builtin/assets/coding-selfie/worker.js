// Built-in MiniApp: Coding Selfie — Node Worker.
// Runs `git` against the current workspace to produce today's coding report.

const { execFile } = require('child_process');
const path = require('path');

const EXT_TO_LANG = {
  '.ts': 'TypeScript', '.tsx': 'TSX', '.js': 'JavaScript', '.jsx': 'JSX', '.mjs': 'JavaScript',
  '.cjs': 'JavaScript', '.py': 'Python', '.rs': 'Rust', '.go': 'Go', '.java': 'Java',
  '.kt': 'Kotlin', '.swift': 'Swift', '.cpp': 'C++', '.cc': 'C++', '.c': 'C', '.h': 'C/C++',
  '.hpp': 'C++', '.cs': 'C#', '.rb': 'Ruby', '.php': 'PHP', '.scala': 'Scala', '.sh': 'Shell',
  '.bash': 'Shell', '.zsh': 'Shell', '.css': 'CSS', '.scss': 'SCSS', '.sass': 'Sass',
  '.less': 'Less', '.html': 'HTML', '.htm': 'HTML', '.json': 'JSON', '.md': 'Markdown',
  '.mdx': 'MDX', '.yml': 'YAML', '.yaml': 'YAML', '.toml': 'TOML', '.ini': 'INI', '.sql': 'SQL',
  '.vue': 'Vue', '.svelte': 'Svelte', '.lua': 'Lua', '.dart': 'Dart', '.r': 'R', '.proto': 'Protobuf',
  '.gradle': 'Gradle', '.tf': 'Terraform', '.hcl': 'HCL', '.ex': 'Elixir', '.exs': 'Elixir',
  '.erl': 'Erlang', '.elm': 'Elm', '.zig': 'Zig', '.nim': 'Nim', '.jl': 'Julia', '.clj': 'Clojure',
  '.cljs': 'ClojureScript', '.fs': 'F#', '.ml': 'OCaml', '.coffee': 'CoffeeScript', '.xml': 'XML',
  '.makefile': 'Make',
};

function langOf(file) {
  const base = path.basename(file).toLowerCase();
  if (base === 'dockerfile' || base.endsWith('.dockerfile')) return 'Docker';
  if (base === 'makefile') return 'Make';
  if (base === 'cmakelists.txt') return 'CMake';
  const ext = path.extname(file).toLowerCase();
  if (!ext) return null;
  return EXT_TO_LANG[ext] || null;
}

function git(cwd, args, opts = {}) {
  return new Promise((resolve, reject) => {
    execFile(
      'git',
      args,
      {
        cwd,
        encoding: 'utf8',
        maxBuffer: opts.maxBuffer || 32 * 1024 * 1024,
        timeout: opts.timeout || 12000,
        env: { ...process.env, GIT_TERMINAL_PROMPT: '0', LC_ALL: 'C' },
      },
      (err, stdout, stderr) => {
        if (err) reject(new Error(((stderr || '').trim() || err.message || String(err))));
        else resolve(stdout);
      },
    );
  });
}

function dayKey(d) {
  const dt = new Date(d);
  return (
    dt.getFullYear() +
    '-' +
    String(dt.getMonth() + 1).padStart(2, '0') +
    '-' +
    String(dt.getDate()).padStart(2, '0')
  );
}

function summarize(cs) {
  let added = 0, deleted = 0;
  const langs = new Map();
  const files = new Set();
  const hours = new Array(24).fill(0);
  for (const c of cs) {
    added += c.added;
    deleted += c.deleted;
    for (const f of c.files) {
      files.add(f.path);
      const l = f.lang;
      if (l) {
        const t = (f.added + f.deleted) || 1;
        langs.set(l, (langs.get(l) || 0) + t);
      }
    }
    const h = new Date(c.date).getHours();
    if (h >= 0 && h < 24) hours[h] += 1;
  }
  const langArr = Array.from(langs.entries())
    .map(([name, weight]) => ({ name, weight }))
    .sort((a, b) => b.weight - a.weight);
  return {
    commitCount: cs.length,
    added,
    deleted,
    fileCount: files.size,
    langs: langArr,
    hours,
    commits: cs.slice(0, 12).map((c) => ({
      hash: c.hash.slice(0, 7),
      date: c.date,
      author: c.author,
      subject: c.subject,
      added: c.added,
      deleted: c.deleted,
    })),
  };
}

module.exports = {
  async 'workspace.scan'(params) {
    const cwd = (params && (params.cwd || params.workspace)) || '';
    if (!cwd) {
      return { ok: false, reason: 'no-workspace' };
    }

    // 1) Verify it is a git work tree.
    try {
      const inside = (await git(cwd, ['rev-parse', '--is-inside-work-tree'])).trim();
      if (inside !== 'true') {
        return { ok: false, reason: 'not-a-git-repo' };
      }
    } catch (_e) {
      return { ok: false, reason: 'not-a-git-repo' };
    }

    // 2) Repo basics.
    const [topLevelRaw, branchRaw, userNameRaw, userEmailRaw] = await Promise.all([
      git(cwd, ['rev-parse', '--show-toplevel']).catch(() => cwd),
      git(cwd, ['rev-parse', '--abbrev-ref', 'HEAD']).catch(() => 'HEAD'),
      git(cwd, ['config', 'user.name']).catch(() => ''),
      git(cwd, ['config', 'user.email']).catch(() => ''),
    ]);
    const topLevel = topLevelRaw.trim() || cwd;
    const branch = branchRaw.trim() || 'HEAD';
    const userName = userNameRaw.trim();
    const userEmail = userEmailRaw.trim();
    const repoName = path.basename(topLevel);

    // 3) Auto-discover every email the current author has committed with in this
    //    repo. This handles the very common case where upstream squash-merges
    //    rewrite your commits to your GitHub no-reply email, or you have
    //    multiple email identities (work / personal / noreply) for the same
    //    name. Falls back to user.email only if name lookup fails.
    const detectedEmails = new Set();
    if (userEmail) detectedEmails.add(userEmail);
    if (userName) {
      try {
        const emailMap = await git(
          topLevel,
          ['log', '--all', '--pretty=format:%aN\t%aE'],
          { timeout: 12000, maxBuffer: 32 * 1024 * 1024 },
        );
        for (const line of emailMap.split('\n')) {
          const tab = line.indexOf('\t');
          if (tab < 0) continue;
          const n = line.slice(0, tab).trim();
          const e = line.slice(tab + 1).trim();
          if (n && e && n === userName) detectedEmails.add(e);
        }
      } catch (_e) {
        // best-effort; fall back to user.email only
      }
    }

    // Build a single --author regex matching the user's name OR any of the
    // detected emails. git --author treats the value as a POSIX BRE.
    const escRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const patterns = [];
    if (userName) patterns.push(escRe(userName));
    for (const e of detectedEmails) patterns.push(escRe(e));
    const authorPattern = patterns.length ? patterns.join('\\|') : '';

    // 4) Pull last 52 weeks of commits with numstat across ALL branches.
    const SEP = '\x1f';
    const REC = '\x1e';
    const fmt = `${REC}%H${SEP}%aI${SEP}%aN${SEP}%aE${SEP}%s`;
    const args = [
      'log',
      '--all',
      '--since=52.weeks',
      '--no-merges',
      `--pretty=format:${fmt}`,
      '--numstat',
    ];
    if (authorPattern) args.push(`--author=${authorPattern}`);

    let raw;
    try {
      raw = await git(topLevel, args, { timeout: 30000, maxBuffer: 64 * 1024 * 1024 });
    } catch (e) {
      return { ok: false, reason: 'git-log-failed', message: String(e.message || e) };
    }

    const commits = [];
    const records = raw.split(REC);
    for (const rec of records) {
      const trimmed = rec.replace(/^\n+/, '');
      if (!trimmed) continue;
      const lines = trimmed.split('\n');
      const header = lines[0] || '';
      const parts = header.split(SEP);
      if (parts.length < 5) continue;
      const [hash, date, author, email, subject] = parts;
      const files = [];
      let added = 0, deleted = 0;
      for (let i = 1; i < lines.length; i++) {
        const ln = lines[i];
        if (!ln) continue;
        const m = ln.match(/^(\d+|-)\t(\d+|-)\t(.+)$/);
        if (!m) continue;
        const a = m[1] === '-' ? 0 : parseInt(m[1], 10);
        const d = m[2] === '-' ? 0 : parseInt(m[2], 10);
        added += a;
        deleted += d;
        files.push({ path: m[3], added: a, deleted: d, lang: langOf(m[3]) });
      }
      commits.push({ hash, date, author, email, subject, added, deleted, files });
    }

    // 4) Aggregate by day (local time).
    const todayStart = new Date();
    todayStart.setHours(0, 0, 0, 0);
    const yesterdayStart = new Date(todayStart);
    yesterdayStart.setDate(todayStart.getDate() - 1);

    const byDay = new Map();
    for (const c of commits) {
      const k = dayKey(c.date);
      if (!byDay.has(k)) byDay.set(k, []);
      byDay.get(k).push(c);
    }

    const todayKey = dayKey(todayStart);
    const yesterdayKey = dayKey(yesterdayStart);
    const today = summarize(byDay.get(todayKey) || []);
    const yesterday = summarize(byDay.get(yesterdayKey) || []);

    const weekCutoff = new Date(todayStart.getTime() - 6 * 86400000);
    const week = summarize(commits.filter((c) => new Date(c.date) >= weekCutoff));
    const month = summarize(commits);

    // 5) Streak: consecutive days backwards with at least one commit. Counts today
    //    if there are commits today; otherwise starts from yesterday.
    let streak = 0;
    const cursor = new Date(todayStart);
    if (!byDay.has(dayKey(cursor))) {
      cursor.setDate(cursor.getDate() - 1);
    }
    while (byDay.has(dayKey(cursor))) {
      streak += 1;
      cursor.setDate(cursor.getDate() - 1);
    }

    // 6) Year heatmap: 7 * 52 = 364 days, oldest → today.
    const HEATMAP_DAYS = 7 * 52;
    const heatmap = [];
    for (let i = HEATMAP_DAYS - 1; i >= 0; i--) {
      const d = new Date(todayStart);
      d.setDate(d.getDate() - i);
      const k = dayKey(d);
      const list = byDay.get(k) || [];
      heatmap.push({ date: k, count: list.length });
    }

    return {
      ok: true,
      repo: { name: repoName, path: topLevel, branch },
      author: {
        name: userName,
        email: userEmail,
        detectedEmails: Array.from(detectedEmails),
        scope: 'all-branches',
      },
      today,
      yesterday,
      week,
      month,
      streak,
      heatmap,
      generatedAt: new Date().toISOString(),
    };
  },
};
