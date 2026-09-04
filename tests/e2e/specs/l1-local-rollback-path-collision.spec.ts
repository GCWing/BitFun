/**
 * L1 regression coverage for local rollback when an SSH workspace has the
 * same absolute path.
 */

import { browser, expect } from '@wdio/globals';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

type WorkspaceInfo = {
  id: string;
  rootPath: string;
};

type InvokeResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: string };

async function invoke<T>(command: string, args: unknown): Promise<InvokeResult<T>> {
  return browser.execute(async (targetCommand: string, targetArgs: unknown) => {
    const tauri = (window as typeof window & {
      __TAURI__?: {
        core?: {
          invoke?: (command: string, args?: unknown) => Promise<unknown>;
        };
      };
    }).__TAURI__;
    const tauriInvoke = tauri?.core?.invoke;
    if (typeof tauriInvoke !== 'function') {
      return { ok: false as const, error: 'Tauri invoke is unavailable' };
    }

    try {
      return {
        ok: true as const,
        value: await tauriInvoke(targetCommand, targetArgs) as T,
      };
    } catch (error) {
      return { ok: false as const, error: String(error) };
    }
  }, command, args);
}

async function invokeExpectingFailure(command: string, args: unknown): Promise<string> {
  const driverPort = Number(process.env.OPENBITFUN_E2E_WEBDRIVER_PORT || 4445);
  const response = await fetch(
    `http://127.0.0.1:${driverPort}/session/${browser.sessionId}/execute/sync`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        script: `return (async (command, args) => {
          return window.__TAURI__.core.invoke(command, args);
        }).apply(null, arguments);`,
        args: [command, args],
      }),
    },
  );
  const payload = await response.json() as {
    value?: { message?: string } | string;
  };
  expect(response.ok).toBe(false);
  return typeof payload.value === 'string'
    ? payload.value
    : payload.value?.message || JSON.stringify(payload.value);
}

describe('L1 Local rollback workspace identity', () => {
  const connectionId = 'e2e-rollback-path-collision';
  let fixtureRoot = '';
  let localWorkspaceId: string | null = null;
  let remoteWorkspaceId: string | null = null;

  before(() => {
    fixtureRoot = fs.realpathSync(
      fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-rollback-collision-e2e-')),
    );
  });

  it('uses explicit local identity when a registered SSH workspace has the same path', async () => {
    const remoteOpen = await invoke<WorkspaceInfo>('open_remote_workspace', {
      request: {
        remotePath: fixtureRoot,
        connectionId,
        connectionName: 'E2E rollback collision',
        sshHost: 'rollback-collision.example',
      },
    });
    expect(remoteOpen.ok).toBe(true);
    if (!remoteOpen.ok) {
      throw new Error(remoteOpen.error);
    }
    remoteWorkspaceId = remoteOpen.value.id;
    expect(remoteWorkspaceId.startsWith('remote_')).toBe(true);

    const localOpen = await invoke<WorkspaceInfo>('open_workspace', {
      request: { path: fixtureRoot },
    });
    expect(localOpen.ok).toBe(true);
    if (!localOpen.ok) {
      throw new Error(localOpen.error);
    }
    localWorkspaceId = localOpen.value.id;
    expect(localWorkspaceId.startsWith('local_')).toBe(true);
    expect(localWorkspaceId).not.toBe(remoteWorkspaceId);

    const baseRequest = {
      workspacePath: fixtureRoot,
      sessionId: 'e2e-rollback-collision-session',
      targetTurnId: 'e2e-rollback-collision-turn',
    };

    const legacyError = await invokeExpectingFailure('rollback_session_to_turn', {
      request: baseRequest,
    });
    expect(legacyError).toContain('not supported for remote workspaces');

    const localIdentityError = await invokeExpectingFailure('rollback_session_to_turn', {
      request: {
        ...baseRequest,
        workspaceId: localWorkspaceId,
        workspaceHostname: 'localhost',
      },
    });
    expect(localIdentityError).not.toContain('not supported for remote workspaces');
    expect(localIdentityError).not.toContain('unavailable for remote workspaces');

    const conflictingRemoteError = await invokeExpectingFailure('rollback_session_to_turn', {
      request: {
        ...baseRequest,
        workspaceId: localWorkspaceId,
        workspaceHostname: 'localhost',
        remoteConnectionId: connectionId,
      },
    });
    expect(conflictingRemoteError).toContain('not supported for remote workspaces');
  });

  after(async () => {
    if (remoteWorkspaceId) {
      await invoke('close_workspace', {
        request: { workspaceId: remoteWorkspaceId },
      });
    } else if (fixtureRoot) {
      await invoke('remote_remove_workspace', {
        connectionId,
        remotePath: fixtureRoot,
      });
    }

    if (localWorkspaceId) {
      await invoke('close_workspace', {
        request: { workspaceId: localWorkspaceId },
      });
    }

    if (
      fixtureRoot
      && path.basename(fixtureRoot).startsWith('openbitfun-rollback-collision-e2e-')
      && fs.existsSync(fixtureRoot)
    ) {
      fs.rmSync(fixtureRoot, { recursive: true, force: true });
    }
  });
});
