import { spawn } from 'child_process';
import { execFileSync } from 'child_process';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { randomUUID } from 'crypto';
import { fileURLToPath } from 'url';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const e2eDirectory = path.resolve(scriptDirectory, '..');
const manifestPath = path.join(
  os.tmpdir(),
  `openbitfun-worktree-restart-manifest-${process.pid}-${randomUUID()}.json`,
);
const pnpmCommand = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm';

function runPhase(phase) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      pnpmCommand,
      [
        'exec',
        'wdio',
        'run',
        './config/wdio.conf.ts',
        '--spec',
        './specs/l1-worktree-restart.spec.ts',
      ],
      {
        cwd: e2eDirectory,
        stdio: 'inherit',
        env: {
          ...process.env,
          E2E_LOG_LEVEL: process.env.E2E_LOG_LEVEL || 'warn',
          OPENBITFUN_E2E_WORKTREE_RESTART_PHASE: phase,
          OPENBITFUN_E2E_WORKTREE_RESTART_MANIFEST: manifestPath,
        },
      },
    );

    child.on('error', reject);
    child.on('exit', (code, signal) => {
      if (code === 0) {
        resolve();
        return;
      }
      reject(new Error(
        `Worktree restart ${phase} phase failed (code=${code}, signal=${signal ?? 'none'})`,
      ));
    });
  });
}

function cleanupAbandonedFixture() {
  if (!fs.existsSync(manifestPath)) {
    return;
  }

  try {
    const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
    const repositoryPath = manifest.repositoryPath;
    const worktreePath = manifest.worktree?.path;
    if (
      typeof repositoryPath === 'string'
      && fs.existsSync(repositoryPath)
      && typeof worktreePath === 'string'
      && fs.existsSync(worktreePath)
    ) {
      try {
        execFileSync(
          'git',
          ['worktree', 'remove', '--force', worktreePath],
          { cwd: repositoryPath, stdio: 'ignore' },
        );
      } catch {
        // The in-app after hook normally performs this cleanup.
      }
    }
    if (
      typeof manifest.fixtureRoot === 'string'
      && path.basename(manifest.fixtureRoot).startsWith(
        'openbitfun-worktree-restart-e2e-',
      )
    ) {
      fs.rmSync(manifest.fixtureRoot, { recursive: true, force: true });
    }
  } finally {
    fs.rmSync(manifestPath, { force: true });
  }
}

try {
  await runPhase('create');
  await runPhase('verify');
} finally {
  cleanupAbandonedFixture();
}
