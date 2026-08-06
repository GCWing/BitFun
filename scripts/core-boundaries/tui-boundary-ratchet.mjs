import { readFileSync, readdirSync, statSync } from 'fs';
import { join, relative } from 'path';

import {
  tuiLegacyBackendBudgets,
  tuiLegacyBackendMarkers,
} from './rules/tui-boundary-rules.mjs';

const scanRoots = [
  'src/apps/cli/src/ui',
  'src/apps/cli/src/modes/chat.rs',
  'src/apps/cli/src/modes/chat',
];

function countLiteral(text, marker) {
  let count = 0;
  let offset = 0;
  while ((offset = text.indexOf(marker, offset)) >= 0) {
    count += 1;
    offset += marker.length;
  }
  return count;
}

function walkFiles(dir, visit) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      walkFiles(path, visit);
    } else {
      visit(path);
    }
  }
}

export function checkTuiLegacyBackendRatchet(root) {
  const failures = [];
  const seenPaths = new Set();

  for (const scanRoot of scanRoots) {
    const path = join(root, ...scanRoot.split('/'));
    const visit = (filePath) => {
      if (!filePath.endsWith('.rs')) return;
      const repoPath = relative(root, filePath).replace(/\\/g, '/');
      if (seenPaths.has(repoPath)) return;
      seenPaths.add(repoPath);
      const text = readFileSync(filePath, 'utf8');

      for (const marker of tuiLegacyBackendMarkers) {
        const actual = countLiteral(text, marker);
        const allowed = tuiLegacyBackendBudgets[repoPath]?.[marker] ?? 0;
        if (actual > allowed) {
          failures.push({
            path: filePath,
            line: 1,
            message:
              `TUI backend direct-call debt must only decrease; marker ${marker} has ${actual} occurrences, budget ${allowed}. Route new backend work through the CLI-local TuiBackend and App Server`,
          });
        }
      }
    };

    if (statSync(path).isDirectory()) walkFiles(path, visit);
    else visit(path);
  }

  return failures;
}
