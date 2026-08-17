/**
 * Native L1 coverage for subagent identity presentation.
 *
 * Creates real persisted session relationships through Desktop commands, then
 * verifies the rendered Agent tree through BitFun's embedded WebDriver.
 */

import { browser, expect, $ } from '@wdio/globals';
import { randomUUID } from 'crypto';
import { Header } from '../page-objects/components/Header';
import { SessionTree } from '../page-objects/components/SessionTree';
import { getWorkspaceState } from '../helpers/workspace-helper';

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

    await invoke('start_dialog_turn', {
      sessionId: rootSessionId,
      userInput: 'Verify the subagent identity presentation.',
      turnId: randomUUID(),
      execution: { kind: 'standard' },
      agentType: 'agentic',
      workspacePath,
    });
    await browser.pause(250);
    await invokeOutcome('cancel_session', { sessionId: rootSessionId });

    await browser.refresh();
    await header.waitForLoad();
    const rootSession = await $(`[data-testid="nav-session-item"][data-session-id="${rootSessionId}"]`);
    await rootSession.waitForClickable({ timeout: 20000 });
    await rootSession.click();
  });

  it('shows stable, non-repeating avatars and localized names in the Agent tree', async () => {
    await sessionTree.open();
    await browser.waitUntil(async () => (await sessionTree.getSubagentIdentities()).length === 3, {
      timeout: 15000,
      interval: 250,
      timeoutMsg: 'The Agent tree did not render all persisted subagent identities',
    });

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
