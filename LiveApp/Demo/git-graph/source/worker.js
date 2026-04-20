// Git Graph Live App — Worker (Node.js/Bun). Uses simple-git npm package.
// Methods are invoked via app.call('git.log', params) etc. from the UI.

const simpleGit = require('simple-git');
const EOL_REGEX = /\r\n|\r|\n/g;
const GIT_LOG_SEP = 'XX7Nal-YARtTpjCikii9nJxER19D6diSyk-AWkPb';

function getGit(cwd) {
  if (!cwd || typeof cwd !== 'string') {
    throw new Error('git: cwd (repository path) is required');
  }
  return simpleGit({ baseDir: cwd });
}

function normalizeLogCommit(c) {
  const parents = Array.isArray(c.parents)
    ? c.parents
    : c.parent
      ? [c.parent]
      : [];
  return {
    hash: c.hash,
    shortHash: c.hash ? c.hash.slice(0, 7) : '',
    message: c.message,
    author: c.author_name,
    email: c.author_email,
    date: c.date,
    refs: c.refs || '',
    parentHashes: parents,
  };
}

/** Parse git show-ref -d --head output into head, heads, tags, remotes. */
async function getRefsFromShowRef(cwd, showRemoteBranches, hideRemotes = []) {
  const git = getGit(cwd);
  const args = ['show-ref'];
  if (!showRemoteBranches) args.push('--heads', '--tags');
  args.push('-d', '--head');
  const stdout = await git.raw(args).catch(() => '');
  const refData = { head: null, heads: [], tags: [], remotes: [] };
  const hidePatterns = hideRemotes.map((r) => 'refs/remotes/' + r + '/');
  const lines = stdout.trim().split(EOL_REGEX).filter(Boolean);
  for (const line of lines) {
    const parts = line.split(' ');
    if (parts.length < 2) continue;
    const hash = parts.shift();
    const ref = parts.join(' ');
    if (ref.startsWith('refs/heads/')) {
      refData.heads.push({ hash, name: ref.substring(11) });
    } else if (ref.startsWith('refs/tags/')) {
      const annotated = ref.endsWith('^{}');
      refData.tags.push({
        hash,
        name: annotated ? ref.substring(10, ref.length - 3) : ref.substring(10),
        annotated,
      });
    } else if (ref.startsWith('refs/remotes/')) {
      if (!hidePatterns.some((p) => ref.startsWith(p)) && !ref.endsWith('/HEAD')) {
        refData.remotes.push({ hash, name: ref.substring(13) });
      }
    } else if (ref === 'HEAD') {
      refData.head = hash;
    }
  }
  return refData;
}

/** Parse git reflog refs/stash --format=... into stashes with baseHash, untrackedFilesHash, selector. */
async function getStashesFromReflog(cwd) {
  const git = getGit(cwd);
  const format = ['%H', '%P', '%gD', '%an', '%ae', '%at', '%s'].join(GIT_LOG_SEP);
  const stdout = await git.raw(['reflog', '--format=' + format, 'refs/stash', '--']).catch(() => '');
  const stashes = [];
  const lines = stdout.trim().split(EOL_REGEX).filter(Boolean);
  for (const line of lines) {
    const parts = line.split(GIT_LOG_SEP);
    if (parts.length < 7 || !parts[1]) continue;
    const parentHashes = parts[1].trim().split(/\s+/);
    stashes.push({
      hash: parts[0],
      baseHash: parentHashes[0],
      untrackedFilesHash: parentHashes.length >= 3 ? parentHashes[2] : null,
      selector: parts[2] || 'stash@{0}',
      author: parts[3] || '',
      email: parts[4] || '',
      date: parseInt(parts[5], 10) || 0,
      message: parts[6] || '',
    });
  }
  return stashes;
}

/** Build uncommitted node from status + diff --name-status + diff --numstat (HEAD to working tree). */
async function getUncommittedNode(cwd, headHash) {
  const git = getGit(cwd);
  const [statusOut, nameStatusOut, numStatOut] = await Promise.all([
    git.raw(['status', '-s', '--porcelain', '-z', '--untracked-files=all']).catch(() => ''),
    git.raw(['diff', '--name-status', '--find-renames', '-z', 'HEAD']).catch(() => ''),
    git.raw(['diff', '--numstat', '--find-renames', '-z', 'HEAD']).catch(() => ''),
  ]);
  const statusLines = statusOut.split('\0').filter((s) => s.length >= 4);
  if (statusLines.length === 0) return null;
  const nameStatusParts = nameStatusOut.split('\0').filter(Boolean);
  const numStatLines = numStatOut.trim().split('\n').filter(Boolean);
  const files = [];
  const numStatByPath = {};
  for (const nl of numStatLines) {
    const m = nl.match(/^(\d+|-)\s+(\d+|-)\s+(.+)$/);
    if (m) numStatByPath[m[3].replace(/\t.*$/, '')] = { additions: m[1] === '-' ? 0 : parseInt(m[1], 10), deletions: m[2] === '-' ? 0 : parseInt(m[2], 10) };
  }
  let i = 0;
  while (i < nameStatusParts.length) {
    const type = nameStatusParts[i][0];
    if (type === 'A' || type === 'M' || type === 'D') {
      const path = nameStatusParts[i + 1] || nameStatusParts[i].slice(2);
      const stat = numStatByPath[path] || { additions: 0, deletions: 0 };
      files.push({ oldFilePath: path, newFilePath: path, type, additions: stat.additions, deletions: stat.deletions });
      i += 2;
    } else if (type === 'R') {
      const oldPath = nameStatusParts[i + 1];
      const newPath = nameStatusParts[i + 2];
      const stat = numStatByPath[newPath] || { additions: 0, deletions: 0 };
      files.push({ oldFilePath: oldPath, newFilePath: newPath, type: 'R', additions: stat.additions, deletions: stat.deletions });
      i += 3;
    } else {
      i += 1;
    }
  }
  return {
    hash: '__uncommitted__',
    shortHash: 'WIP',
    message: 'Uncommitted Changes (' + statusLines.length + ')',
    author: '',
    email: '',
    date: Math.round(Date.now() / 1000),
    parentHashes: headHash ? [headHash] : [],
    heads: [],
    tags: [],
    remotes: [],
    stash: null,
    isUncommitted: true,
    changeCount: statusLines.length,
    files,
  };
}

module.exports = {
  // ─── Log & show ───────────────────────────────────────────────────────────
  async 'git.log'({ cwd, maxCount = 100, order = 'date', firstParent = false, branches = [] }) {
    const git = getGit(cwd);
    const n = Math.min(Math.max(1, Number(maxCount) || 100), 1000);
    const args = ['-n', String(n)];
    if (order === 'topo') args.push('--topo-order');
    else if (order === 'author-date') args.push('--author-date-order');
    else args.push('--date-order');
    if (firstParent) args.push('--first-parent');
    if (Array.isArray(branches) && branches.length > 0) {
      args.push(...branches);
      args.push('--');
    }
    const log = await git.log(args);
    return {
      all: (log.all || []).map(normalizeLogCommit),
      latest: log.latest ? normalizeLogCommit(log.latest) : null,
    };
  },

  /**
   * Aggregated graph data: head, commits (with heads/tags/remotes/stash), refs, stashes, uncommitted, remotes, status.
   * UI should consume this instead of assembling from git.log + git.branches + git.stashList + status.
   */
  async 'git.graphData'({
    cwd,
    maxCount = 300,
    order = 'date',
    firstParent = false,
    branches = [],
    showRemoteBranches = true,
    showStashes = true,
    showUncommittedChanges = true,
    hideRemotes = [],
  }) {
    const git = getGit(cwd);
    const n = Math.min(Math.max(1, Number(maxCount) || 300), 1000);
    let refData = { head: null, heads: [], tags: [], remotes: [] };
    let stashes = [];
    try {
      refData = await getRefsFromShowRef(cwd, showRemoteBranches, hideRemotes);
    } catch (_) {}
    if (showStashes) {
      try {
        stashes = await getStashesFromReflog(cwd);
      } catch (_) {}
    }
    const logArgs = ['-n', String(n + 1)];
    if (order === 'topo') logArgs.push('--topo-order');
    else if (order === 'author-date') logArgs.push('--author-date-order');
    else logArgs.push('--date-order');
    if (firstParent) logArgs.push('--first-parent');
    if (Array.isArray(branches) && branches.length > 0) {
      logArgs.push(...branches);
      logArgs.push('--');
    } else {
      logArgs.push('--branches', '--tags', 'HEAD');
      stashes.forEach((s) => {
        if (s.baseHash && !logArgs.includes(s.baseHash)) logArgs.push(s.baseHash);
      });
    }
    const log = await git.log(logArgs);
    let rawCommits = (log.all || []).map(normalizeLogCommit);
    const moreCommitsAvailable = rawCommits.length > n;
    if (moreCommitsAvailable) rawCommits = rawCommits.slice(0, n);
    const commitLookup = {};
    rawCommits.forEach((c, i) => { commitLookup[c.hash] = i; });
    const commits = rawCommits.map((c) => ({
      ...c,
      heads: [],
      tags: [],
      remotes: [],
      stash: null,
    }));
    stashes.forEach((s) => {
      if (typeof commitLookup[s.hash] === 'number') {
        commits[commitLookup[s.hash]].stash = {
          selector: s.selector,
          baseHash: s.baseHash,
          untrackedFilesHash: s.untrackedFilesHash,
        };
      }
    });
    const toAdd = [];
    stashes.forEach((s) => {
      if (typeof commitLookup[s.hash] === 'number') return;
      if (typeof commitLookup[s.baseHash] !== 'number') return;
      toAdd.push({ index: commitLookup[s.baseHash], data: s });
    });
    toAdd.sort((a, b) => (a.index !== b.index ? a.index - b.index : b.data.date - a.data.date));
    for (let i = toAdd.length - 1; i >= 0; i--) {
      const s = toAdd[i].data;
      commits.splice(toAdd[i].index, 0, {
        hash: s.hash,
        shortHash: s.hash ? s.hash.slice(0, 7) : '',
        message: s.message,
        author: s.author,
        email: s.email,
        date: s.date,
        parentHashes: [s.baseHash],
        heads: [],
        tags: [],
        remotes: [],
        stash: { selector: s.selector, baseHash: s.baseHash, untrackedFilesHash: s.untrackedFilesHash },
      });
    }
    for (let i = 0; i < commits.length; i++) commitLookup[commits[i].hash] = i;
    refData.heads.forEach((h) => {
      if (typeof commitLookup[h.hash] === 'number') commits[commitLookup[h.hash]].heads.push(h.name);
    });
    refData.tags.forEach((t) => {
      if (typeof commitLookup[t.hash] === 'number') commits[commitLookup[t.hash]].tags.push({ name: t.name, annotated: t.annotated });
    });
    refData.remotes.forEach((r) => {
      if (typeof commitLookup[r.hash] === 'number') {
        const remote = r.name.indexOf('/') >= 0 ? r.name.split('/')[0] : null;
        commits[commitLookup[r.hash]].remotes.push({ name: r.name, remote });
      }
    });
    let uncommitted = null;
    if (showUncommittedChanges && refData.head) {
      const headInList = commits.some((c) => c.hash === refData.head);
      if (headInList) {
        try {
          uncommitted = await getUncommittedNode(cwd, refData.head);
          if (uncommitted) commits.unshift(uncommitted);
        } catch (_) {}
      }
    }
    let status = null;
    try {
      status = await git.status();
      status = {
        current: status.current,
        tracking: status.tracking,
        not_added: status.not_added || [],
        staged: status.staged || [],
        modified: status.modified || [],
        created: status.created || [],
        deleted: status.deleted || [],
        renamed: status.renamed || [],
        files: status.files || [],
      };
    } catch (_) {}
    let remotes = [];
    try {
      const remotesMap = await git.getRemotes(true);
      remotes = Object.entries(remotesMap || {}).map(([name, r]) => ({
        name,
        fetch: (r && r.fetch) || '',
        push: (r && r.push) || '',
      }));
    } catch (_) {}
    return {
      head: refData.head,
      commits,
      refs: refData,
      stashes,
      uncommitted: uncommitted ? { changeCount: uncommitted.changeCount, files: uncommitted.files } : null,
      remotes,
      status,
      moreCommitsAvailable,
    };
  },

  async 'git.searchCommits'({ cwd, query, maxCount = 100 }) {
    const git = getGit(cwd);
    const n = Math.min(Math.max(1, Number(maxCount) || 100), 500);
    const log = await git.log(['-n', String(n), '--grep', String(query), '--all']);
    return { all: (log.all || []).map(normalizeLogCommit) };
  },

  async 'git.branches'({ cwd }) {
    const git = getGit(cwd);
    const branch = await git.branch();
    return {
      current: branch.current,
      all: branch.all || [],
      branches: branch.branches || {},
    };
  },

  async 'git.status'({ cwd }) {
    const git = getGit(cwd);
    const status = await git.status();
    return {
      current: status.current,
      tracking: status.tracking,
      not_added: status.not_added || [],
      staged: status.staged || [],
      modified: status.modified || [],
      created: status.created || [],
      deleted: status.deleted || [],
      renamed: status.renamed || [],
      files: status.files || [],
    };
  },

  async 'git.show'({ cwd, hash }) {
    if (!hash) throw new Error('git.show: hash is required');
    const git = getGit(cwd);
    const log = await git.log([hash, '-n', '1']);
    const commit = log.latest;
    if (!commit) return { commit: null, files: [] };
    let files = [];
    try {
      const summary = await git.diffSummary([hash + '^..' + hash]);
      if (summary && summary.files) {
        files = summary.files.map((f) => ({
          file: f.file,
          changes: f.changes || 0,
          insertions: f.insertions || 0,
          deletions: f.deletions || 0,
        }));
      }
    } catch (_) {}
    return {
      commit: {
        hash: commit.hash,
        shortHash: commit.hash ? commit.hash.slice(0, 7) : '',
        message: commit.message,
        body: commit.body || '',
        author: commit.author_name,
        email: commit.author_email,
        date: commit.date,
        refs: commit.refs || '',
      },
      files,
    };
  },

  // ─── Checkout & branch ────────────────────────────────────────────────────
  async 'git.checkout'({ cwd, ref, createBranch = null }) {
    const git = getGit(cwd);
    if (createBranch) {
      await git.checkoutLocalBranch(createBranch, ref);
      return { branch: createBranch };
    }
    await git.checkout(ref);
    return { ref };
  },

  async 'git.createBranch'({ cwd, name, startPoint, checkout = false }) {
    const git = getGit(cwd);
    if (checkout) {
      await git.checkoutLocalBranch(name, startPoint);
    } else {
      await git.branch([name, startPoint]);
    }
    return { name };
  },

  async 'git.deleteBranch'({ cwd, name, force = false }) {
    const git = getGit(cwd);
    await git.deleteLocalBranch(name, force);
    return { deleted: name };
  },

  async 'git.renameBranch'({ cwd, oldName, newName }) {
    const git = getGit(cwd);
    await git.raw(['branch', '-m', oldName, newName]);
    return { newName };
  },

  // ─── Merge & rebase ─────────────────────────────────────────────────────────
  async 'git.merge'({ cwd, ref, noFF = false, squash = false, noCommit = false }) {
    const git = getGit(cwd);
    const args = [ref];
    if (noFF) args.unshift('--no-ff');
    if (squash) args.unshift('--squash');
    if (noCommit) args.unshift('--no-commit');
    await git.merge(args);
    return { merged: ref };
  },

  async 'git.rebase'({ cwd, onto, branch = null }) {
    const git = getGit(cwd);
    if (branch) {
      await git.rebase([branch]);
    } else {
      await git.rebase([onto]);
    }
    return { rebased: onto || branch };
  },

  // ─── Push & pull & fetch ───────────────────────────────────────────────────
  async 'git.push'({ cwd, remote, branch, setUpstream = false, force = false, forceWithLease = false }) {
    const git = getGit(cwd);
    const args = [remote];
    if (branch) args.push(branch);
    if (setUpstream) args.push('--set-upstream');
    if (force) args.push('--force');
    if (forceWithLease) args.push('--force-with-lease');
    await git.push(args);
    return { pushed: true };
  },

  async 'git.pull'({ cwd, remote, branch, noFF = false, squash = false }) {
    const git = getGit(cwd);
    const args = [remote];
    if (branch) args.push(branch);
    if (noFF) args.push('--no-ff');
    if (squash) args.push('--squash');
    await git.pull(args);
    return { pulled: true };
  },

  async 'git.fetch'({ cwd, remote, prune = false, pruneTags = false }) {
    const git = getGit(cwd);
    const args = remote ? [remote] : [];
    if (prune) args.push('--prune');
    if (pruneTags) args.push('--prune-tags');
    await git.fetch(args);
    return { fetched: true };
  },

  async 'git.fetchIntoLocalBranch'({ cwd, remote, remoteBranch, localBranch, force = false }) {
    const git = getGit(cwd);
    const ref = `${remote}/${remoteBranch}:refs/heads/${localBranch}`;
    const args = [remote, ref];
    if (force) args.push('--force');
    await git.fetch(args);
    return { localBranch };
  },

  // ─── Commit operations ─────────────────────────────────────────────────────
  async 'git.cherryPick'({ cwd, hash, noCommit = false, recordOrigin = false }) {
    const git = getGit(cwd);
    const args = ['cherry-pick'];
    if (noCommit) args.push('--no-commit');
    if (recordOrigin) args.push('-x');
    args.push(hash);
    await git.raw(args);
    return { hash };
  },

  async 'git.revert'({ cwd, hash, parentIndex = null }) {
    const git = getGit(cwd);
    const args = ['revert', '--no-edit'];
    if (parentIndex != null) args.push('-m', String(parentIndex));
    args.push(hash);
    await git.raw(args);
    return { hash };
  },

  async 'git.reset'({ cwd, hash, mode = 'mixed' }) {
    const git = getGit(cwd);
    const modes = { soft: 'soft', mixed: 'mixed', hard: 'hard' };
    const m = modes[mode] || 'mixed';
    await git.reset([m, hash]);
    return { hash, mode: m };
  },

  async 'git.dropCommit'({ cwd, hash }) {
    const git = getGit(cwd);
    await git.raw(['rebase', '--onto', hash + '^', hash]);
    return { hash };
  },

  // ─── Tags ──────────────────────────────────────────────────────────────────
  async 'git.tags'({ cwd }) {
    const git = getGit(cwd);
    const tags = await git.tags();
    return { all: tags.all || [] };
  },

  async 'git.addTag'({ cwd, name, ref, annotated = false, message = null }) {
    const git = getGit(cwd);
    if (annotated && message != null) {
      if (ref) {
        await git.raw(['tag', '-a', name, ref, '-m', message]);
      } else {
        await git.addAnnotatedTag(name, message);
      }
    } else {
      if (ref) {
        await git.raw(['tag', name, ref]);
      } else {
        await git.addTag(name);
      }
    }
    return { name };
  },

  async 'git.deleteTag'({ cwd, name }) {
    const git = getGit(cwd);
    await git.raw(['tag', '-d', name]);
    return { deleted: name };
  },

  async 'git.pushTag'({ cwd, remote, name }) {
    const git = getGit(cwd);
    await git.push(remote, `refs/tags/${name}`);
    return { name };
  },

  async 'git.tagDetails'({ cwd, name }) {
    const git = getGit(cwd);
    try {
      const show = await git.raw(['show', '--no-patch', name]);
      return { output: show };
    } catch (e) {
      return { output: null, error: e.message };
    }
  },

  // ─── Stash ─────────────────────────────────────────────────────────────────
  async 'git.stashList'({ cwd }) {
    const git = getGit(cwd);
    const list = await git.stashList();
    const items = (list.all || []).map((s) => ({
      hash: s.hash,
      shortHash: s.hash ? s.hash.slice(0, 7) : '',
      message: s.message,
      date: s.date,
      refs: s.refs || '',
      parentHashes: Array.isArray(s.parents) ? s.parents : s.parent ? [s.parent] : [],
      stashSelector: s.hash ? `stash@{${list.all.indexOf(s)}}` : null,
    }));
    return { all: items };
  },

  async 'git.stashPush'({ cwd, message = null, includeUntracked = false }) {
    const git = getGit(cwd);
    const args = ['push'];
    if (message) args.push('-m', message);
    if (includeUntracked) args.push('--include-untracked');
    await git.stash(args);
    return { pushed: true };
  },

  async 'git.stashApply'({ cwd, selector, restoreIndex = false }) {
    const git = getGit(cwd);
    const args = ['apply'];
    if (restoreIndex) args.push('--index');
    args.push(selector);
    await git.stash(args);
    return { applied: selector };
  },

  async 'git.stashPop'({ cwd, selector, restoreIndex = false }) {
    const git = getGit(cwd);
    const args = ['pop'];
    if (restoreIndex) args.push('--index');
    args.push(selector);
    await git.stash(args);
    return { popped: selector };
  },

  async 'git.stashDrop'({ cwd, selector }) {
    const git = getGit(cwd);
    await git.stash(['drop', selector]);
    return { dropped: selector };
  },

  async 'git.stashBranch'({ cwd, branchName, selector }) {
    const git = getGit(cwd);
    await git.stash(['branch', branchName, selector]);
    return { branch: branchName };
  },

  // ─── Remotes ────────────────────────────────────────────────────────────────
  async 'git.remotes'({ cwd }) {
    const git = getGit(cwd);
    const remotes = await git.getRemotes(true);
    const list = Object.entries(remotes || {}).map(([name, r]) => ({
      name,
      fetch: (r && r.fetch) || '',
      push: (r && r.push) || '',
    }));
    return { remotes: list };
  },

  async 'git.addRemote'({ cwd, name, url, pushUrl = null }) {
    const git = getGit(cwd);
    await git.addRemote(name, url);
    if (pushUrl) {
      await git.raw(['remote', 'set-url', '--push', name, pushUrl]);
    }
    return { name };
  },

  async 'git.removeRemote'({ cwd, name }) {
    const git = getGit(cwd);
    await git.removeRemote(name);
    return { removed: name };
  },

  async 'git.setRemoteUrl'({ cwd, name, url, push = false }) {
    const git = getGit(cwd);
    if (push) {
      await git.raw(['remote', 'set-url', '--push', name, url]);
    } else {
      await git.raw(['remote', 'set-url', name, url]);
    }
    return { name };
  },

  // ─── Diff & compare ────────────────────────────────────────────────────────
  async 'git.fileDiff'({ cwd, from, to, file }) {
    const git = getGit(cwd);
    const args = ['--unified=3', from, to, '--', file];
    const out = await git.diff(args);
    return { diff: out };
  },

  async 'git.compareCommits'({ cwd, hash1, hash2 }) {
    const git = getGit(cwd);
    const out = await git.raw(['diff', '--name-status', hash1, hash2]);
    const lines = (out || '').trim().split('\n').filter(Boolean);
    const files = lines.map((line) => {
      const m = line.match(/^([AMD])\s+(.+)$/) || line.match(/^([AR])\d*\s+(.+)\s+(.+)$/);
      if (m) {
        const status = m[1];
        const path = m[2] || line.slice(2).trim();
        return { status, file: path };
      }
      return { status: '?', file: line.trim() };
    });
    return { files };
  },

  // ─── Uncommitted / clean ───────────────────────────────────────────────────
  async 'git.resetUncommitted'({ cwd, mode = 'mixed' }) {
    const git = getGit(cwd);
    const modes = { soft: 'soft', mixed: 'mixed', hard: 'hard' };
    await git.reset([modes[mode] || 'mixed', 'HEAD']);
    return { reset: true };
  },

  async 'git.cleanUntracked'({ cwd, force = false, directories = false }) {
    const git = getGit(cwd);
    const args = ['clean'];
    if (force) args.push('-f');
    if (directories) args.push('-fd');
    await git.raw(args);
    return { cleaned: true };
  },
};
