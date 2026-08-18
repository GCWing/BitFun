/**
 * Native L1 coverage for subagent identity presentation.
 *
 * Creates real persisted session relationships through Desktop commands, then
 * verifies the rendered Agent tree through BitFun's embedded WebDriver.
 */

import { browser, expect, $ } from '@wdio/globals';
import { randomUUID } from 'crypto';
import { mkdtemp, rm } from 'fs/promises';
import { tmpdir } from 'os';
import { join } from 'path';
import { Header } from '../page-objects/components/Header';
import { SessionTree } from '../page-objects/components/SessionTree';
import { getWorkspaceState, openWorkspaceThroughFrontend } from '../helpers/workspace-helper';
import { saveElementScreenshot } from '../helpers/screenshot-utils';

type InvokeOutcome<T> =
  | { ok: true; value: T }
  | { ok: false; error: unknown };

interface CreatedSession {
  sessionId: string;
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
            invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
          };
        };
      };
      const invoke = tauriWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== 'function') {
        done(JSON.stringify({ ok: false, error: { message: 'Tauri invoke is unavailable' } }));
        return;
      }
      invoke(commandName, { request: commandRequest }).then(
        value => done(JSON.stringify({ ok: true, value })),
        error => done(JSON.stringify({
          ok: false,
          error: typeof error === 'string' ? { message: error } : error,
        })),
      );
    },
    command,
    request,
  );
  return JSON.parse(encoded as string) as InvokeOutcome<T>;
}

async function invoke<T>(command: string, request: Record<string, unknown>): Promise<T> {
  const outcome = await invokeOutcome<T>(command, request);
  if (!outcome.ok) {
    throw new Error(`${command} failed: ${JSON.stringify(outcome.error)}`);
  }
  return outcome.value;
}

describe('L1 Subagent identity', () => {
  const header = new Header();
  const sessionTree = new SessionTree();
  const createdSessionIds: string[] = [];
  let workspacePath = '';
  let rootSessionId = '';

  before(async () => {
    await header.waitForLoad();
    const workspaceState = await getWorkspaceState();
    workspacePath = workspaceState.currentWorkspacePath ?? '';
    if (!workspacePath) {
      throw new Error('The native E2E profile must have an active workspace');
    }

    rootSessionId = randomUUID();
    const root = await invoke<CreatedSession>('create_session', {
      sessionId: rootSessionId,
      sessionName: 'Subagent identity E2E',
      agentType: 'agentic',
      workspacePath,
      sessionKind: 'standard',
    });
    createdSessionIds.push(root.sessionId);

    const agentTypes = ['Explore', 'Review', 'Research'];
    for (let index = 0; index < agentTypes.length; index += 1) {
      const childSessionId = randomUUID();
      const child = await invoke<CreatedSession>('create_session', {
        sessionId: childSessionId,
        sessionName: `${agentTypes[index]}: identity presentation ${index + 1}`,
        agentType: agentTypes[index],
        workspacePath,
        sessionKind: 'subagent',
        relationship: {
          kind: 'subagent',
          parentSessionId: rootSessionId,
          parentDialogTurnId: randomUUID(),
          parentToolCallId: randomUUID(),
          subagentType: agentTypes[index],
        },
      });
      createdSessionIds.push(child.sessionId);
    }

    const fixtureTimestamp = Date.now();
    await invoke('save_session_turn', {
      turn_data: {
        turnId: randomUUID(),
        turnIndex: 0,
        sessionId: rootSessionId,
        timestamp: fixtureTimestamp,
        kind: 'local_command',
        agentType: 'agentic',
        userMessage: {
          id: randomUUID(),
          content: '# Session overview E2E fixture',
          timestamp: fixtureTimestamp,
          metadata: {
            localCommandKind: 'usage_report',
          },
        },
        modelRounds: [],
        startTime: fixtureTimestamp,
        endTime: fixtureTimestamp,
        durationMs: 0,
        status: 'completed',
        hasFinalResponse: false,
      },
      workspace_path: workspacePath,
    });

    await browser.refresh();
    await header.waitForLoad();
    const rootSession = await $(`[data-testid="nav-session-item"][data-session-id="${rootSessionId}"]`);
    await rootSession.waitForClickable({ timeout: 20000 });
    await rootSession.click();
  });

  it('shows one default status list with the real Agent tree', async () => {
    const overviewPanel = await sessionTree.openOverview();
    const overviewItems = await overviewPanel.$$('[data-bf-part="sessionOverviewItem"]');
    expect(overviewItems).toHaveLength(3);
    expect(await overviewItems[0].getText()).toMatch(/Agents/);
    expect(await overviewItems[1].getText()).toMatch(/后台终端|背景終端|Background terminals/);
    expect(await overviewItems[2].getText()).toMatch(/拉取请求|拉取請求|Pull requests/);
    expect((await overviewItems[2].getCSSProperty('border-bottom-width')).value).toBe('0px');
    expect(await overviewPanel.$('[data-testid="flowchat-header-session-overview-back"]').isExisting()).toBe(false);
    expect(await sessionTree.getBackgroundEmptyStateText()).toMatch(
      /暂无后台终端|目前沒有背景終端|No background terminals/,
    );
    await browser.waitUntil(async () => (await sessionTree.getSubagentIdentities()).length === 3, {
      timeout: 15000,
      interval: 250,
      timeoutMsg: 'The default status list did not render all persisted subagent identities',
    });
    const agentRows = await overviewPanel.$$('.session-tree-popover__node--subagent');
    expect(agentRows).toHaveLength(3);
    for (const agentRow of agentRows) {
      expect(await agentRow.getSize('height')).toBeLessThanOrEqual(34);
      expect((await agentRow.$('.session-tree-popover__node-copy')
        .getCSSProperty('flex-direction')).value).toBe('row');
    }
    const pullRequestState = await sessionTree.waitForPullRequestOverviewState();
    if (pullRequestState === 'loaded') {
      const pullRequestItemCount = await sessionTree.getPullRequestItemCount();
      expect((await sessionTree.hasPullRequestEmptyState()) || pullRequestItemCount > 0).toBe(true);
      if (pullRequestItemCount > 0) {
        expect(await sessionTree.pullRequestItemsHaveDetailIndicators()).toBe(true);
      }
    }
    await saveElementScreenshot(
      '[data-testid="flowchat-header-session-overview-panel"]',
      'l1-session-overview-list',
    );

    const identities = await sessionTree.getSubagentIdentities();
    expect(identities).toHaveLength(3);
    expect(new Set(identities.map(identity => identity.avatarId)).size).toBe(3);
    expect(new Set(identities.map(identity => identity.nameId)).size).toBe(3);
    for (const identity of identities) {
      expect(identity.avatarId).toMatch(/^robot-\d{2}$/);
      expect(identity.nameId).toMatch(/^name-\d{2}$/);
      expect(identity.name.length).toBeGreaterThan(0);
      expect(identity.imageSource).toContain('robot-');
    }
  });

  it('opens the session right panel from the header control', async () => {
    await sessionTree.closeOverview();
    await sessionTree.setRightPanelOpen(false);
    expect(await sessionTree.getRightPanelControlState()).toEqual({
      open: false,
      state: 'collapsed',
      hasOpenIcon: true,
      hasCloseIcon: false,
    });

    try {
      await sessionTree.setRightPanelOpen(true);
      expect(await sessionTree.getRightPanelControlState()).toEqual({
        open: true,
        state: 'open',
        hasOpenIcon: false,
        hasCloseIcon: true,
      });
      await saveElementScreenshot(
        '[data-testid="session-scene"]',
        'l1-session-right-panel-control',
      );
    } finally {
      await sessionTree.setRightPanelOpen(false);
    }
  });

  it('shows pull requests as unavailable in a real non-Git workspace', async () => {
    const nonGitWorkspacePath = await mkdtemp(join(tmpdir(), 'bitfun-e2e-non-git-'));
    const nonGitSessionId = randomUUID();

    try {
      await openWorkspaceThroughFrontend(nonGitWorkspacePath);
      expect(await invoke<boolean>('git_is_repository', {
        repositoryPath: nonGitWorkspacePath,
      })).toBe(false);

      await invoke<CreatedSession>('create_session', {
        sessionId: nonGitSessionId,
        sessionName: 'Non-Git session overview E2E',
        agentType: 'agentic',
        workspacePath: nonGitWorkspacePath,
        sessionKind: 'standard',
      });

      const fixtureTimestamp = Date.now();
      await invoke('save_session_turn', {
        turn_data: {
          turnId: randomUUID(),
          turnIndex: 0,
          sessionId: nonGitSessionId,
          timestamp: fixtureTimestamp,
          kind: 'local_command',
          agentType: 'agentic',
          userMessage: {
            id: randomUUID(),
            content: '# Non-Git session overview E2E fixture',
            timestamp: fixtureTimestamp,
            metadata: {
              localCommandKind: 'usage_report',
            },
          },
          modelRounds: [],
          startTime: fixtureTimestamp,
          endTime: fixtureTimestamp,
          durationMs: 0,
          status: 'completed',
          hasFinalResponse: false,
        },
        workspace_path: nonGitWorkspacePath,
      });

      await browser.refresh();
      await header.waitForLoad();
      const nonGitSession = await $(`[data-testid="nav-session-item"][data-session-id="${nonGitSessionId}"]`);
      await nonGitSession.waitForClickable({ timeout: 20000 });
      await nonGitSession.click();

      await sessionTree.openOverview();
      expect(await sessionTree.waitForPullRequestOverviewState()).toBe('unavailable');
      expect(await sessionTree.getPullRequestUnavailableStateText()).toMatch(
        /当前工作区不是 Git 仓库|目前工作區不是 Git 倉庫|This workspace is not a Git repository/,
      );
      await saveElementScreenshot(
        '[data-testid="flowchat-header-session-overview-panel"]',
        'l1-session-overview-non-git',
      );
    } finally {
      await invokeOutcome('archive_session', {
        session_id: nonGitSessionId,
        workspace_path: nonGitWorkspacePath,
      });
      await invokeOutcome('delete_session', {
        sessionId: nonGitSessionId,
        workspacePath: nonGitWorkspacePath,
      });
      try {
        await openWorkspaceThroughFrontend(workspacePath);
      } finally {
        await rm(nonGitWorkspacePath, {
          recursive: true,
          force: true,
          maxRetries: 3,
          retryDelay: 100,
        });
      }
    }
  });

  after(async () => {
    for (const sessionId of [...createdSessionIds].reverse()) {
      await invokeOutcome('archive_session', {
        session_id: sessionId,
        workspace_path: workspacePath,
      });
      await invokeOutcome('delete_session', { sessionId, workspacePath });
    }
  });
});
