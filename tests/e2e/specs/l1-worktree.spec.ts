/**
 * L1 managed Worktree workflow.
 *
 * Exercises the real desktop UI, Tauri commands, Git worktrees, session
 * persistence, and removal guards against a temporary repository.
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

interface ToolExecutionResponse {
  tool_name: string;
  success: boolean;
  result?: {
    success?: boolean;
    operation?: string;
    worktree_id?: string;
    path?: string;
    session_id?: string;
  };
  error?: string;
  validation_error?: string;
}

type InvokeOutcome<T> =
  | { ok: true; value: T }
  | { ok: false; error: unknown };

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

/** Create a fresh Code session in the active project workspace. */
async function createSessionThroughUI(): Promise<void> {
  const newSessionButton = await $('[data-testid="nav-new-code-session-btn"]');
  await newSessionButton.waitForClickable({ timeout: 15000 });
  await newSessionButton.click();
}

/**
 * Flip the worktree chip next to the branch in the chat input and wait for the
 * rebind to land. The chip is the only entry point for session isolation.
 */
async function toggleSessionWorktree(enabled: boolean): Promise<void> {
  const toggle = await $('[data-testid="chat-input-worktree-toggle"]');
  await toggle.waitForClickable({ timeout: 20000 });
  await browser.waitUntil(
    async () => (await toggle.getAttribute('data-worktree-enabled')) === String(!enabled),
    {
      timeout: 20000,
      interval: 200,
      timeoutMsg: `Worktree toggle was not ${enabled ? 'off' : 'on'} before switching`,
    },
  );
  await toggle.click();
  await browser.waitUntil(
    async () => (await toggle.getAttribute('data-worktree-enabled')) === String(enabled),
    {
      timeout: 30000,
      interval: 250,
      timeoutMsg: `Worktree toggle did not turn ${enabled ? 'on' : 'off'}`,
    },
  );
}

function errorCode(outcome: InvokeOutcome<unknown>): string | undefined {
  if (outcome.ok || !outcome.error || typeof outcome.error !== 'object') {
    return undefined;
  }
  return (outcome.error as { code?: string }).code;
}

describe('L1 managed Worktree workflow', () => {
  let fixtureRoot = '';
  let repositoryPath = '';
  let baseCommit = '';
  let createdWorktrees: WorktreeSummary[] = [];

  before(async () => {
    fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-worktree-e2e-'));
    repositoryPath = path.join(fixtureRoot, 'repository');
    fs.mkdirSync(repositoryPath);
    // Desktop canonicalizes local workspace paths. On macOS, os.tmpdir()
    // commonly resolves through /var -> /private/var, so use the canonical
    // path for all frontend/backend identity assertions.
    repositoryPath = fs.realpathSync(repositoryPath);
    git(repositoryPath, ['init']);
    git(repositoryPath, ['config', 'user.name', 'OpenBitFun E2E']);
    git(repositoryPath, ['config', 'user.email', 'openbitfun-e2e@example.invalid']);
    fs.writeFileSync(path.join(repositoryPath, 'shared.txt'), 'base\n');
    git(repositoryPath, ['add', 'shared.txt']);
    git(repositoryPath, ['commit', '-m', 'base']);
    baseCommit = git(repositoryPath, ['rev-parse', 'HEAD']);

    await openWorkspace(repositoryPath);
    await waitForWorkspaceReady(repositoryPath, path.basename(repositoryPath));
  });

  it('creates two isolated sessions from the same baseline through the UI', async () => {
    for (let expectedCount = 1; expectedCount <= 2; expectedCount += 1) {
      await createSessionThroughUI();
      await toggleSessionWorktree(true);

      await browser.waitUntil(async () => {
        const worktrees = await invoke<WorktreeSummary[]>('worktree_list', {
          projectWorkspacePath: repositoryPath,
        });
        return worktrees.filter(worktree => !worktree.isMain).length === expectedCount;
      }, {
        timeout: 30000,
        interval: 250,
        timeoutMsg: `Expected ${expectedCount} managed Worktree(s)`,
      });
    }

    const worktrees = await invoke<WorktreeSummary[]>('worktree_list', {
      projectWorkspacePath: repositoryPath,
    });
    createdWorktrees = worktrees.filter(worktree => !worktree.isMain);

    expect(createdWorktrees).toHaveLength(2);
    expect(new Set(createdWorktrees.map(worktree => worktree.path)).size).toBe(2);
    for (const worktree of createdWorktrees) {
      expect(worktree.head).toBe(baseCommit);
      expect(worktree.branch).toBeUndefined();
      expect(worktree.lifecycle).toBe('managed');
      expect(worktree.missing).toBe(false);
      expect(worktree.sessions).toHaveLength(1);
    }
  });

  it('keeps parallel file changes isolated from each other and the main checkout', () => {
    const [first, second] = createdWorktrees;
    fs.writeFileSync(path.join(first.path, 'shared.txt'), 'first worktree\n');
    fs.writeFileSync(path.join(second.path, 'shared.txt'), 'second worktree\n');

    expect(fs.readFileSync(path.join(repositoryPath, 'shared.txt'), 'utf8')).toBe('base\n');
    expect(fs.readFileSync(path.join(first.path, 'shared.txt'), 'utf8')).toBe(
      'first worktree\n',
    );
    expect(fs.readFileSync(path.join(second.path, 'shared.txt'), 'utf8')).toBe(
      'second worktree\n',
    );
  });

  it('lets the deferred Agent Worktree tool create an isolated child session', async () => {
    const response = await invoke<ToolExecutionResponse>('execute_tool', {
      toolName: 'Worktree',
      input: {
        operation: 'create_session',
        base_ref: 'HEAD',
        copy_local_changes: false,
        session_name: 'Agent-created Worktree session',
        agent_type: 'agentic',
      },
      workspacePath: repositoryPath,
      context: null,
      safeMode: false,
    });

    expect(response.success).toBe(true);
    expect(response.error).toBeNull();
    expect(response.validation_error).toBeNull();
    expect(response.result?.success).toBe(true);
    expect(response.result?.operation).toBe('create_session');
    expect(response.result?.worktree_id).toBeTruthy();
    expect(response.result?.session_id).toBeTruthy();

    const worktrees = await invoke<WorktreeSummary[]>('worktree_list', {
      projectWorkspacePath: repositoryPath,
    });
    const created = worktrees.find(
      worktree => worktree.worktreeId === response.result?.worktree_id,
    );
    expect(created).toBeDefined();
    expect(created?.path).toBe(response.result?.path);
    expect(created?.head).toBe(baseCommit);
    expect(created?.lifecycle).toBe('managed');
    expect(created?.sessions[0]?.sessionId).toBe(response.result?.session_id);
    createdWorktrees.push(created as WorktreeSummary);
  });

  it('restores project grouping and session bindings after a frontend reload', async () => {
    await browser.refresh();
    await waitForWorkspaceReady(repositoryPath, path.basename(repositoryPath));

    const worktreeIds = new Set(createdWorktrees.map(worktree => worktree.worktreeId));
    const worktreeSessionIds = createdWorktrees.map(worktree => worktree.sessions[0].sessionId);
    await browser.waitUntil(async () => {
      const renderedSessionIds = await browser.execute(() => (
        Array.from(document.querySelectorAll('[data-testid="nav-session-item"]'))
          .map(element => element.getAttribute('data-session-id'))
          .filter((value): value is string => Boolean(value))
      ));
      return worktreeSessionIds.every(sessionId => renderedSessionIds.includes(sessionId));
    }, {
      timeout: 20000,
      interval: 250,
      timeoutMsg: 'Worktree sessions were not restored under the project after reload',
    });

    const reconciled = await invoke<WorktreeSummary[]>('worktree_list', {
      projectWorkspacePath: repositoryPath,
    });
    const restored = reconciled.filter(worktree => worktreeIds.has(worktree.worktreeId));
    expect(restored).toHaveLength(createdWorktrees.length);
    for (const worktree of restored) {
      expect(worktree.sessions).toHaveLength(1);
      const metadata = await invoke<Record<string, unknown> | null>(
        'load_persisted_session_metadata',
        {
          session_id: worktree.sessions[0].sessionId,
          workspace_path: repositoryPath,
        },
      );
      expect(metadata).not.toBeNull();
      expect(metadata?.workspacePath).toBe(worktree.path);
      expect(metadata?.projectWorkspacePath).toBe(repositoryPath);
    }
  });

  it('blocks safe removal for dirty and unpublished detached worktrees', async () => {
    for (const worktree of createdWorktrees) {
      await invoke<void>('archive_session', {
        session_id: worktree.sessions[0].sessionId,
        workspace_path: repositoryPath,
      });
    }

    const [dirtyWorktree, unpublishedWorktree] = createdWorktrees;
    git(unpublishedWorktree.path, ['add', 'shared.txt']);
    git(unpublishedWorktree.path, ['commit', '-m', 'detached unpublished change']);

    const dirtyRemoval = await invokeOutcome('worktree_remove', {
      requestId: randomUUID(),
      projectWorkspacePath: repositoryPath,
      worktreeId: dirtyWorktree.worktreeId,
      force: false,
    });
    const unpublishedRemoval = await invokeOutcome('worktree_remove', {
      requestId: randomUUID(),
      projectWorkspacePath: repositoryPath,
      worktreeId: unpublishedWorktree.worktreeId,
      force: false,
    });

    expect(dirtyRemoval.ok).toBe(false);
    expect(errorCode(dirtyRemoval)).toBe('dirty_worktree');
    expect(unpublishedRemoval.ok).toBe(false);
    expect(errorCode(unpublishedRemoval)).toBe('unpublished_commits');
  });

  after(async () => {
    if (repositoryPath) {
      let worktrees = createdWorktrees;
      try {
        worktrees = (await invoke<WorktreeSummary[]>('worktree_list', {
          projectWorkspacePath: repositoryPath,
        })).filter(worktree => !worktree.isMain);
      } catch {
        // Fall back to the last successfully reconciled list.
      }

      for (const worktree of worktrees) {
        for (const session of worktree.sessions) {
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
            // The fixture root cleanup below is limited to this test's directory.
          }
        }
      }

      try {
        git(repositoryPath, ['worktree', 'prune']);
      } catch {
        // Repository cleanup below is sufficient if Git is already unavailable.
      }
    }

    if (
      fixtureRoot
      && path.basename(fixtureRoot).startsWith('openbitfun-worktree-e2e-')
    ) {
      fs.rmSync(fixtureRoot, { recursive: true, force: true });
    }
  });
});
