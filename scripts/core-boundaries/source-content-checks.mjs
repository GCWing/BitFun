import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

export function listTrackedRustRepoPaths(root) {
  const result = spawnSync(
    'git',
    ['-C', root, 'ls-files', '-z', '--', '*.rs'],
    { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
  );
  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || 'git ls-files failed');
  }
  return result.stdout
    .split('\0')
    .filter(Boolean)
    // `git ls-files` continues to report tracked files deleted only in the
    // working tree. Boundary scans must evaluate the prospective checkout
    // without requiring callers to stage deletions first.
    .filter((repoPath) => existsSync(join(root, ...repoPath.split('/'))));
}

function stripRustComments(text) {
  const chars = text.split('');
  let state = 'code';
  let blockDepth = 0;
  let rawTerminator = '';

  const blank = (index) => {
    if (chars[index] !== '\r' && chars[index] !== '\n') {
      chars[index] = ' ';
    }
  };

  for (let index = 0; index < chars.length; index += 1) {
    const current = chars[index];
    const next = chars[index + 1];

    if (state === 'line-comment') {
      if (current === '\n') {
        state = 'code';
      } else {
        blank(index);
      }
      continue;
    }
    if (state === 'block-comment') {
      if (current === '/' && next === '*') {
        blank(index);
        blank(index + 1);
        blockDepth += 1;
        index += 1;
      } else if (current === '*' && next === '/') {
        blank(index);
        blank(index + 1);
        blockDepth -= 1;
        index += 1;
        if (blockDepth === 0) {
          state = 'code';
        }
      } else {
        blank(index);
      }
      continue;
    }
    if (state === 'string') {
      if (current === '\\') {
        index += 1;
      } else if (current === '"') {
        state = 'code';
      }
      continue;
    }
    if (state === 'raw-string') {
      if (text.startsWith(rawTerminator, index)) {
        index += rawTerminator.length - 1;
        state = 'code';
      }
      continue;
    }

    if (current === '/' && next === '/') {
      blank(index);
      blank(index + 1);
      state = 'line-comment';
      index += 1;
    } else if (current === '/' && next === '*') {
      blank(index);
      blank(index + 1);
      state = 'block-comment';
      blockDepth = 1;
      index += 1;
    } else if (current === '"') {
      state = 'string';
    } else if (current === 'r' || (current === 'b' && next === 'r')) {
      let cursor = current === 'r' ? index + 1 : index + 2;
      while (chars[cursor] === '#') {
        cursor += 1;
      }
      if (chars[cursor] === '"') {
        rawTerminator = `"${'#'.repeat(cursor - index - (current === 'r' ? 1 : 2))}`;
        state = 'raw-string';
        index = cursor;
      }
    }
  }
  return chars.join('');
}

function isAllowedWholeFileMatch(text, match, pattern, repoPath) {
  const lineStart = text.lastIndexOf('\n', match.index - 1) + 1;
  const nextNewline = text.indexOf('\n', match.index);
  const lineEnd = nextNewline === -1 ? text.length : nextNewline;
  const lineText = text.slice(lineStart, lineEnd).trim();
  return pattern.allowLines?.some(
    (allowed) => allowed.path === repoPath && allowed.text === lineText,
  ) ?? false;
}

export function findForbiddenContentMatches(text, patterns, repoPath) {
  const matches = [];
  const lines = text.split(/\r?\n/);
  for (const pattern of patterns) {
    if (pattern.allowPaths?.includes(repoPath)) {
      continue;
    }
    if (pattern.wholeFile) {
      const searchableText = pattern.ignoreRustComments ? stripRustComments(text) : text;
      pattern.regex.lastIndex = 0;
      let match = pattern.regex.exec(searchableText);
      while (match) {
        if (!isAllowedWholeFileMatch(text, match, pattern, repoPath)) {
          matches.push({
            line: text.slice(0, match.index).split(/\r?\n/).length,
            message: pattern.message,
          });
        }
        if (!pattern.regex.global) {
          break;
        }
        if (match[0].length === 0) {
          pattern.regex.lastIndex += 1;
        }
        match = pattern.regex.exec(searchableText);
      }
      continue;
    }
    lines.forEach((line, index) => {
      pattern.regex.lastIndex = 0;
      if (pattern.regex.test(line)) {
        matches.push({ line: index + 1, message: pattern.message });
      }
    });
  }
  return matches;
}

export function scanForbiddenContentUnder(root, rule, trackedRustRepoPaths) {
  const trackedPaths = trackedRustRepoPaths ?? listTrackedRustRepoPaths(root);
  const prefix = rule.path === '.' ? '' : `${rule.path.replace(/\/$/, '')}/`;
  const findings = [];
  for (const repoPath of trackedPaths) {
    if (prefix && !repoPath.startsWith(prefix)) {
      continue;
    }
    const path = join(root, ...repoPath.split('/'));
    const text = readFileSync(path, 'utf8');
    for (const match of findForbiddenContentMatches(text, rule.patterns, repoPath)) {
      findings.push({ repoPath, path, ...match });
    }
  }
  return findings;
}
