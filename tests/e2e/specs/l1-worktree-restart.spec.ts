/**
 * Two-process Worktree persistence verification.
 *
 * The companion runner executes this spec twice. The create phase leaves a
 * test-owned repository and managed Worktree in place, then WDIO terminates
 * the Desktop process. The verify phase starts a fresh Desktop process against
 * the same isolated E2E profile, verifies reconciliation, and cleans up.
 */

import { browser, expect, $ } from '@wdio/globals';
import { execFileSync } from 'child_process';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { randomUUID } from 'crypto';
import { waitForWorkspaceReady } from '../helpers/workspace-helper';

interface WorktreeSessionSummary {
  sessionId: string;
  sessionName: string;
  status: string;
  archived: boolean;
}

interface WorktreeSummary {
  worktreeId: string;
  projectWorkspacePath: string;
  path: string;
  head: string;
  branch?: string;
  lifecycle: 'managed' | 'permanent' | 'external';
  isMain: boolean;
  dirty: boolean;
  locked: boolean;
  missing: boolean;
  hasUnpublishedCommits: boolean;
  associatedSessionCount: number;
  runningSessionCount: number;
  sessions: WorktreeSessionSummary[];
}

interface RestartManifest {
  fixtureRoot: string;
  repositoryPath: string;
  baseCommit: string;
  worktree: WorktreeSummary;
}

type InvokeOutcome<T> =
  | { ok: true; value: T }
  | { ok: false; error: unknown };

const phase = process.env.OPENBITFUN_E2E_WORKTREE_RESTART_PHASE;
const manifestPath = process.env.OPENBITFUN_E2E_WORKTREE_RESTART_MANIFEST;

function git(repositoryPath: string, args: string[]): string {
  return execFileSync('git', args, {
    cwd: repositoryPath,
    encoding: 'utf8',
    env: {
      ...process.env,
      GIT_TERMINAL_PROMPT: '0',
    },
  }).trim();
}

async function invokeOutcome<T>(
  command: string,
  request: Record<string, unknown>,
): Promise<InvokeOutcome<T>> {
  const encoded = await browser.executeAsync(
    (
      commandName: string,
      commandRequest: Record<string, unknown>,
      done: (value: string) => void,
    ) => {
      const tauriWindow = window as typeof window & {
        __TAURI__?: {
          core?: {
            invoke?: (
              command: string,
              args?: Record<string, unknown>,
            ) => Promise<unknown>;
          };
        };
      };
      const invoke = tauriWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== 'function') {
        done(JSON.stringify({
          ok: false as const,
          error: { message: 'Tauri invoke is unavailable' },
        }));
        return;
      }

      invoke(commandName, { request: commandRequest }).then(value => {
        done(JSON.stringify({ ok: true as const, value }));
      }, error => {
        let normalizedError: unknown;
        if (typeof error === 'string') {
          try {
            normalizedError = JSON.parse(error);
          } catch {
            normalizedError = { message: error };
          }
        } else if (error && typeof error === 'object') {
          try {
            normalizedError = JSON.parse(JSON.stringify(error));
          } catch {
            normalizedError = { message: String(error) };
          }
        } else {
          normalizedError = { message: String(error) };
        }
        done(JSON.stringify({ ok: false as const, error: normalizedError }));
      });
    },
    command,
    request,
  );
  return JSON.parse(encoded as string) as InvokeOutcome<T>;
}

async function invoke<T>(
  command: string,
  request: Record<string, unknown>,
): Promise<T> {
  const outcome = await invokeOutcome<T>(command, request);
  if (!outcome.ok) {
    throw new Error(`${command} failed: ${JSON.stringify(outcome.error)}`);
  }
  return outcome.value;
}

async function openWorkspace(workspacePath: string): Promise<void> {
  await browser.execute(async (targetWorkspacePath: string) => {
    const { workspaceManager } = await import(
      '/src/infrastructure/services/business/workspaceManager.ts'
    );
    await workspaceManager.openWorkspace(targetWorkspacePath);
  }, workspacePath);
}

async function cleanupFixture(manifest: Partial<RestartManifest>): Promise<void> {
  const repositoryPath = manifest.repositoryPath;
  if (repositoryPath && fs.existsSync(repositoryPath)) {
    let worktrees: WorktreeSummary[] = manifest.worktree ? [manifest.worktree] : [];
    try {
      worktrees = (await invoke<WorktreeSummary[]>('worktree_list', {
        projectWorkspacePath: repositoryPath,
      })).filter(worktree => !worktree.isMain);
    } catch {
      // Fall back to the manifest if Desktop reconciliation is unavailable.
    }

    for (const worktree of worktrees) {
      for (const session of worktree.sessions ?? []) {
        await invokeOutcome('archive_session', {
          session_id: session.sessionId,
          workspace_path: repositoryPath,
        });
        await invokeOutcome('delete_session', {
          sessionId: session.sessionId,
          workspacePath: repositoryPath,
        });
      }
      const removed = await invokeOutcome('worktree_remove', {
        requestId: randomUUID(),
        projectWorkspacePath: repositoryPath,
        worktreeId: worktree.worktreeId,
        force: true,
      });
      if (!removed.ok && fs.existsSync(worktree.path)) {
        try {
          git(repositoryPath, ['worktree', 'remove', '--force', worktree.path]);
        } catch {
          // The fixture cleanup below remains limited to test-owned paths.
        }
      }
    }

    try {
      git(repositoryPath, ['worktree', 'prune']);
    } catch {
      // The repository may already have been removed by a failed setup.
    }
  }

  if (
    manifest.fixtureRoot
    && path.basename(manifest.fixtureRoot).startsWith('openbitfun-worktree-restart-e2e-')
  ) {
    fs.rmSync(manifest.fixtureRoot, { recursive: true, force: true });
  }
}

describe('L1 managed Worktree desktop restart recovery', () => {
  if (!manifestPath) {
    it('requires a runner-provided manifest path', () => {
      throw new Error('OPENBITFUN_E2E_WORKTREE_RESTART_MANIFEST is required');
    });
    return;
  }

  if (phase === 'create') {
    let pendingManifest: Partial<RestartManifest> = {};
    let handoffReady = false;

    it('persists a managed Worktree and session before Desktop exits', async () => {
      const fixtureRoot = fs.mkdtempSync(
        path.join(os.tmpdir(), 'openbitfun-worktree-restart-e2e-'),
      );
      const createdRepositoryPath = path.join(fixtureRoot, 'repository');
      fs.mkdirSync(createdRepositoryPath);
      const repositoryPath = fs.realpathSync(createdRepositoryPath);
      pendingManifest = { fixtureRoot, repositoryPath };

      git(repositoryPath, ['init']);
      git(repositoryPath, ['config', 'user.name', 'OpenBitFun E2E']);
      git(repositoryPath, ['config', 'user.email', 'openbitfun-e2e@example.invalid']);
      fs.writeFileSync(path.join(repositoryPath, 'shared.txt'), 'restart baseline\n');
      git(repositoryPath, ['add', 'shared.txt']);
      git(repositoryPath, ['commit', '-m', 'restart baseline']);
      const baseCommit = git(repositoryPath, ['rev-parse', 'HEAD']);

      await openWorkspace(repositoryPath);
      await waitForWorkspaceReady(repositoryPath, path.basename(repositoryPath));

      const newSessionButton = await $('[data-testid="nav-new-code-session-btn"]');
      await newSessionButton.waitForClickable({ timeout: 15000 });
      await newSessionButton.click();

      const worktreeToggle = await $('[data-testid="chat-input-worktree-toggle"]');
      await worktreeToggle.waitForClickable({ timeout: 20000 });
      await worktreeToggle.click();
      await browser.waitUntil(
        async () => (await worktreeToggle.getAttribute('data-worktree-enabled')) === 'true',
        {
          timeout: 30000,
          interval: 250,
          timeoutMsg: 'Worktree toggle did not turn on',
        },
      );

      const worktrees = await invoke<WorktreeSummary[]>('worktree_list', {
        projectWorkspacePath: repositoryPath,
      });
      const worktree = worktrees.find(candidate => !candidate.isMain);
      expect(worktree).toBeDefined();
      expect(worktree?.head).toBe(baseCommit);
      expect(worktree?.sessions).toHaveLength(1);

      const manifest: RestartManifest = {
        fixtureRoot,
        repositoryPath,
        baseCommit,
        worktree: worktree as WorktreeSummary,
      };
      fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2), {
        encoding: 'utf8',
        flag: 'wx',
      });
      pendingManifest = manifest;
      handoffReady = true;
    });

    after(async () => {
      if (!handoffReady) {
        await cleanupFixture(pendingManifest);
        fs.rmSync(manifestPath, { force: true });
      }
    });
    return;
  }

  if (phase === 'verify') {
    let manifest: RestartManifest;

    before(() => {
      manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8')) as RestartManifest;
    });

    it('recovers the Worktree registry, UI group, and session binding in a new process', async () => {
      await waitForWorkspaceReady(
        manifest.repositoryPath,
        path.basename(manifest.repositoryPath),
        20000,
      );

      const worktrees = await invoke<WorktreeSummary[]>('worktree_list', {
        projectWorkspacePath: manifest.repositoryPath,
      });
      const restored = worktrees.find(
        worktree => worktree.worktreeId === manifest.worktree.worktreeId,
      );
      expect(restored).toBeDefined();
      expect(restored?.path).toBe(manifest.worktree.path);
      expect(restored?.head).toBe(manifest.baseCommit);
      expect(restored?.lifecycle).toBe('managed');
      expect(restored?.missing).toBe(false);
      expect(restored?.sessions).toHaveLength(1);
      expect(restored?.sessions[0].sessionId).toBe(
        manifest.worktree.sessions[0].sessionId,
      );

      const metadata = await invoke<Record<string, unknown> | null>(
        'load_persisted_session_metadata',
        {
          session_id: manifest.worktree.sessions[0].sessionId,
          workspace_path: manifest.repositoryPath,
        },
      );
      expect(metadata?.workspacePath).toBe(manifest.worktree.path);
      expect(metadata?.projectWorkspacePath).toBe(manifest.repositoryPath);

      await browser.waitUntil(async () => browser.execute(
        (worktreeId: string) => Boolean(
          document.querySelector(`[data-worktree-id="${worktreeId}"]`),
        ),
        manifest.worktree.worktreeId,
      ), {
        timeout: 20000,
        interval: 250,
        timeoutMsg: 'Recovered Worktree group was not rendered after Desktop restart',
      });
    });

    after(async () => {
      await cleanupFixture(manifest);
      fs.rmSync(manifestPath, { force: true });
    });
    return;
  }

  it('requires a valid restart phase', () => {
    throw new Error(
      'OPENBITFUN_E2E_WORKTREE_RESTART_PHASE must be "create" or "verify"',
    );
  });
});
