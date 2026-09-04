import { $, browser, expect } from '@wdio/globals';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

type InvokeResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: string };

type MCPJsonConfigSnapshot = {
  jsonConfig: string;
  fingerprint: string;
};

type MCPServerInfo = {
  id: string;
  status: string;
  serverType: string;
  transport: string;
  url?: string;
  oauthEnabled?: boolean;
};

const specDirectory = path.dirname(fileURLToPath(import.meta.url));
const delayedServerFixture = path.resolve(
  specDirectory,
  '..',
  'fixtures',
  'mcp-delayed-server.mjs',
);

async function invoke<T>(command: string, args: unknown = {}): Promise<InvokeResult<T>> {
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

async function expectInvoke<T>(command: string, args: unknown = {}): Promise<T> {
  const result = await invoke<T>(command, args);
  expect(result.ok).toBe(true);
  if (!result.ok) throw new Error(result.error);
  return result.value;
}

async function waitForServerStatus(
  serverId: string,
  expectedStatuses: string[],
  timeout = 15000,
): Promise<string> {
  let lastStatus = '';
  await browser.waitUntil(async () => {
    const status = await invoke<string>('get_mcp_server_status', { serverId });
    if (!status.ok) {
      lastStatus = `invoke error: ${status.error}`;
      return false;
    }
    lastStatus = status.value;
    return expectedStatuses.includes(status.value);
  }, {
    timeout,
    interval: 100,
    timeoutMsg: `${serverId} did not reach ${expectedStatuses.join(' or ')}; last status=${lastStatus}`,
  });
  return lastStatus;
}

function readNonEmptyLines(filePath: string): string[] {
  if (!fs.existsSync(filePath)) return [];
  return fs.readFileSync(filePath, 'utf8').trim().split('\n').filter(Boolean);
}

function processIsAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

async function invokeExpectingFailure(
  command: string,
  args: unknown,
): Promise<{ error: string; durationMs: number }> {
  const driverPort = Number(process.env.OPENBITFUN_E2E_WEBDRIVER_PORT || 4445);
  const startedAt = Date.now();
  const response = await fetch(
    `http://127.0.0.1:${driverPort}/session/${browser.sessionId}/execute/sync`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        script: `return window.__TAURI__.core.invoke(arguments[0], arguments[1]);`,
        args: [command, args],
      }),
    },
  );
  const durationMs = Date.now() - startedAt;
  const payload = await response.json() as {
    value?: { message?: string } | string;
  };
  expect(response.ok).toBe(false);
  const error = typeof payload.value === 'string'
    ? payload.value
    : payload.value?.message || JSON.stringify(payload.value);
  return { error, durationMs };
}

async function openMCPSettings(): Promise<void> {
  const settingsTrigger = await $('[data-testid="nav-footer-settings-item"]');
  await settingsTrigger.waitForClickable({ timeout: 15000 });
  await settingsTrigger.click();

  const openSettings = await $('[data-testid="nav-settings-open-item"]');
  await openSettings.waitForClickable({ timeout: 10000 });
  await openSettings.click();

  const mcpPage = await $('[data-testid="settings-nav-page"][data-settings-page="tools.mcp"]');
  await mcpPage.waitForClickable({ timeout: 15000 });
  await mcpPage.click();

  await $('[data-openbitfun-component="mcp-tools-config"][data-openbitfun-part="root"]')
    .waitForDisplayed({ timeout: 15000 });
}

describe('L1 MCP lifecycle', () => {
  let originalConfig = '';
  let counterPath = '';
  let fastCounterPath = '';
  let slowStopCounterPath = '';
  let slowStopEventPath = '';
  let switchCounterPath = '';

  before(async () => {
    counterPath = path.join(
      os.tmpdir(),
      `openbitfun-mcp-lifecycle-${process.pid}-${Date.now()}.log`,
    );
    fastCounterPath = path.join(
      os.tmpdir(),
      `openbitfun-mcp-lifecycle-fast-${process.pid}-${Date.now()}.log`,
    );
    slowStopCounterPath = path.join(
      os.tmpdir(),
      `openbitfun-mcp-lifecycle-slow-stop-${process.pid}-${Date.now()}.log`,
    );
    slowStopEventPath = path.join(
      os.tmpdir(),
      `openbitfun-mcp-lifecycle-slow-stop-events-${process.pid}-${Date.now()}.log`,
    );
    switchCounterPath = path.join(
      os.tmpdir(),
      `openbitfun-mcp-lifecycle-switch-${process.pid}-${Date.now()}.log`,
    );

    const snapshot = await expectInvoke<MCPJsonConfigSnapshot>('load_mcp_json_config');
    originalConfig = snapshot.jsonConfig;

    const testConfig = JSON.stringify({
      mcpServers: {
        'e2e-delayed': {
          name: 'E2E Delayed MCP',
          type: 'stdio',
          command: process.execPath,
          args: [delayedServerFixture, counterPath, '3000'],
          enabled: true,
          autoStart: false,
        },
        'e2e-fast': {
          name: 'E2E Fast MCP',
          type: 'stdio',
          command: process.execPath,
          args: [delayedServerFixture, fastCounterPath, '0'],
          enabled: true,
          autoStart: false,
        },
        'e2e-slow-stop': {
          name: 'E2E Slow Stop MCP',
          type: 'stdio',
          command: process.execPath,
          args: [delayedServerFixture, slowStopCounterPath, '0', '450', slowStopEventPath],
          enabled: true,
          autoStart: false,
        },
        'e2e-switch': {
          name: 'E2E Switch MCP',
          type: 'stdio',
          command: process.execPath,
          args: [delayedServerFixture, switchCounterPath, '0'],
          enabled: true,
          autoStart: false,
        },
        notion: {
          name: 'Notion',
          type: 'streamable-http',
          url: 'https://mcp.notion.com/mcp',
          enabled: true,
          autoStart: false,
        },
      },
    });

    await expectInvoke<void>('save_mcp_json_config', {
      jsonConfig: testConfig,
      expectedFingerprint: snapshot.fingerprint,
    });

    await openMCPSettings();
  });

  it('shows Notion as a native remote MCP and opens authorization in one click', async () => {
    const notionRow = await $('[data-testid="mcp-server-item"][data-server-id="notion"]');
    await notionRow.waitForDisplayed({ timeout: 10000 });

    const notion = (await expectInvoke<MCPServerInfo[]>('get_mcp_servers'))
      .find((server) => server.id === 'notion');
    expect(notion).toBeDefined();
    expect(notion?.serverType).toBe('Remote');
    expect(notion?.transport).toBe('streamable-http');
    expect(notion?.url).toBe('https://mcp.notion.com/mcp');
    expect(notion?.oauthEnabled).toBe(true);

    const authButton = await notionRow.$('[data-testid="mcp-server-auth"]');
    await authButton.waitForClickable({ timeout: 10000 });
    await authButton.click();

    const authEditor = await $('[data-openbitfun-component="mcp-tools-config"][data-openbitfun-part="authEditor"]');
    await authEditor.waitForDisplayed({ timeout: 5000 });
    expect(await authEditor.isDisplayed()).toBe(true);

    await browser.keys('Escape');
    await authEditor.waitForDisplayed({ reverse: true, timeout: 5000 });
  });

  it('coalesces repeated start clicks while starting a different MCP independently', async () => {
    const delayedRow = await $('[data-testid="mcp-server-item"][data-server-id="e2e-delayed"]');
    await delayedRow.waitForDisplayed({ timeout: 10000 });
    const startButton = await delayedRow.$('[data-testid="mcp-server-start"]');
    await startButton.waitForClickable({ timeout: 10000 });
    const fastRow = await $('[data-testid="mcp-server-item"][data-server-id="e2e-fast"]');
    const fastStartButton = await fastRow.$('[data-testid="mcp-server-start"]');
    await fastStartButton.waitForClickable({ timeout: 10000 });

    const fastStartedAt = Date.now();
    await browser.execute(() => {
      const delayedButton = document.querySelector<HTMLButtonElement>(
        '[data-testid="mcp-server-item"][data-server-id="e2e-delayed"] [data-testid="mcp-server-start"]',
      );
      const fastButton = document.querySelector<HTMLButtonElement>(
        '[data-testid="mcp-server-item"][data-server-id="e2e-fast"] [data-testid="mcp-server-start"]',
      );
      delayedButton?.click();
      delayedButton?.click();
      fastButton?.click();
    });

    await browser.waitUntil(async () => !(await startButton.isEnabled()), {
      timeout: 3000,
      timeoutMsg: 'start button did not become disabled while the MCP was starting',
    });

    const duplicate = await invokeExpectingFailure(
      'start_mcp_server',
      { serverId: 'e2e-delayed' },
    );
    expect(duplicate.error).toContain('already in progress');
    expect(duplicate.durationMs).toBeLessThan(1000);

    await waitForServerStatus('e2e-fast', ['Connected', 'Healthy'], 2000);
    expect(Date.now() - fastStartedAt).toBeLessThan(2000);
    const fastStarts = readNonEmptyLines(fastCounterPath);
    expect(fastStarts).toHaveLength(1);

    await waitForServerStatus('e2e-delayed', ['Connected', 'Healthy']);

    const starts = readNonEmptyLines(counterPath);
    expect(starts).toHaveLength(1);
  });

  it('restarts one MCP while a different MCP is still starting', async () => {
    const delayedRow = await $('[data-testid="mcp-server-item"][data-server-id="e2e-delayed"]');
    const delayedStopButton = await delayedRow.$('[data-testid="mcp-server-stop"]');
    await delayedStopButton.waitForClickable({ timeout: 10000 });
    await delayedStopButton.click();
    await waitForServerStatus('e2e-delayed', ['Stopped']);
    await waitForServerStatus('e2e-fast', ['Connected', 'Healthy']);
    const fastStartsBeforeRestart = readNonEmptyLines(fastCounterPath).length;
    const delayedStartsBeforeRestart = readNonEmptyLines(counterPath).length;

    const delayedStartButton = await delayedRow.$('[data-testid="mcp-server-start"]');
    await delayedStartButton.waitForClickable({ timeout: 10000 });
    await delayedStartButton.click();
    await browser.waitUntil(() => (
      readNonEmptyLines(counterPath).length === delayedStartsBeforeRestart + 1
    ), {
      timeout: 2000,
      interval: 25,
      timeoutMsg: 'delayed MCP process was not spawned',
    });
    const delayedPid = Number(readNonEmptyLines(counterPath).at(-1));

    const restartedAt = Date.now();
    await expectInvoke<void>('restart_mcp_server', { serverId: 'e2e-fast' });
    expect(Date.now() - restartedAt).toBeLessThan(2000);
    await waitForServerStatus('e2e-fast', ['Connected', 'Healthy'], 2000);
    expect(readNonEmptyLines(fastCounterPath)).toHaveLength(fastStartsBeforeRestart + 1);
    expect(processIsAlive(delayedPid)).toBe(true);

    await waitForServerStatus('e2e-delayed', ['Connected', 'Healthy']);
  });

  it('coalesces rapid stop clicks and lets another MCP stop and start during shutdown', async () => {
    const slowStopRow = await $('[data-testid="mcp-server-item"][data-server-id="e2e-slow-stop"]');
    const switchRow = await $('[data-testid="mcp-server-item"][data-server-id="e2e-switch"]');
    const slowStartButton = await slowStopRow.$('[data-testid="mcp-server-start"]');
    const switchStartButton = await switchRow.$('[data-testid="mcp-server-start"]');
    await slowStartButton.waitForClickable({ timeout: 10000 });
    await switchStartButton.waitForClickable({ timeout: 10000 });

    await browser.execute(() => {
      document.querySelector<HTMLButtonElement>(
        '[data-testid="mcp-server-item"][data-server-id="e2e-slow-stop"] [data-testid="mcp-server-start"]',
      )?.click();
      document.querySelector<HTMLButtonElement>(
        '[data-testid="mcp-server-item"][data-server-id="e2e-switch"] [data-testid="mcp-server-start"]',
      )?.click();
    });
    await waitForServerStatus('e2e-slow-stop', ['Connected', 'Healthy']);
    await waitForServerStatus('e2e-switch', ['Connected', 'Healthy']);

    const slowStopButton = await slowStopRow.$('[data-testid="mcp-server-stop"]');
    await slowStopButton.waitForClickable({ timeout: 5000 });
    await browser.execute(() => {
      const button = document.querySelector<HTMLButtonElement>(
        '[data-testid="mcp-server-item"][data-server-id="e2e-slow-stop"] [data-testid="mcp-server-stop"]',
      );
      button?.click();
      button?.click();
    });

    if (process.platform !== 'win32') {
      await browser.waitUntil(() => readNonEmptyLines(slowStopEventPath).length > 0, {
        timeout: 2000,
        interval: 25,
        timeoutMsg: 'slow-stop MCP did not receive its termination signal',
      });
      const slowStopPid = Number(readNonEmptyLines(slowStopCounterPath).at(-1));
      expect(processIsAlive(slowStopPid)).toBe(true);
      const duplicate = await invokeExpectingFailure(
        'stop_mcp_server',
        { serverId: 'e2e-slow-stop' },
      );
      expect(duplicate.error).toContain('already in progress');
      expect(duplicate.durationMs).toBeLessThan(1000);
    }

    await expectInvoke<void>('stop_mcp_server', { serverId: 'e2e-switch' });
    await waitForServerStatus('e2e-switch', ['Stopped']);

    const switchStartsBeforeRestart = readNonEmptyLines(switchCounterPath).length;
    await expectInvoke<void>('start_mcp_server', { serverId: 'e2e-switch' });
    await waitForServerStatus('e2e-switch', ['Connected', 'Healthy']);
    expect(readNonEmptyLines(switchCounterPath)).toHaveLength(switchStartsBeforeRestart + 1);
    if (process.platform !== 'win32') {
      const slowStopPid = Number(readNonEmptyLines(slowStopCounterPath).at(-1));
      expect(processIsAlive(slowStopPid)).toBe(true);
      expect(readNonEmptyLines(slowStopEventPath)).toHaveLength(1);
      await browser.waitUntil(() => !processIsAlive(slowStopPid), {
        timeout: 2000,
        interval: 25,
        timeoutMsg: 'slow-stop MCP process did not exit after its shutdown delay',
      });
    }

    await waitForServerStatus('e2e-slow-stop', ['Stopped']);
  });

  after(async () => {
    await invoke<void>('stop_mcp_server', { serverId: 'e2e-delayed' });
    await invoke<void>('stop_mcp_server', { serverId: 'e2e-fast' });
    await invoke<void>('stop_mcp_server', { serverId: 'e2e-slow-stop' });
    await invoke<void>('stop_mcp_server', { serverId: 'e2e-switch' });
    const current = await invoke<MCPJsonConfigSnapshot>('load_mcp_json_config');
    if (current.ok && originalConfig) {
      await invoke<void>('save_mcp_json_config', {
        jsonConfig: originalConfig,
        expectedFingerprint: current.value.fingerprint,
      });
    }
    if (counterPath && fs.existsSync(counterPath)) {
      fs.rmSync(counterPath);
    }
    if (fastCounterPath && fs.existsSync(fastCounterPath)) {
      fs.rmSync(fastCounterPath);
    }
    if (slowStopCounterPath && fs.existsSync(slowStopCounterPath)) {
      fs.rmSync(slowStopCounterPath);
    }
    if (slowStopEventPath && fs.existsSync(slowStopEventPath)) {
      fs.rmSync(slowStopEventPath);
    }
    if (switchCounterPath && fs.existsSync(switchCounterPath)) {
      fs.rmSync(switchCounterPath);
    }
  });
});
