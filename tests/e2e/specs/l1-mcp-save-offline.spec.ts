import { $, browser, expect } from '@wdio/globals';
import { createServer, type Server } from 'node:http';
import { mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { MCPSettingsPage } from '../page-objects/MCPSettingsPage';

type Snapshot = { jsonConfig: string; fingerprint: string };

async function invoke<T>(command: string, args: unknown = {}): Promise<T> {
  return browser.execute(async (name: string, params: unknown) => {
    const host = window as typeof window & {
      __TAURI__: { core: { invoke: (name: string, params: unknown) => Promise<T> } };
    };
    return host.__TAURI__.core.invoke(name, params);
  }, command, args);
}

describe('L1 MCP save while the endpoint is unavailable', () => {
  const page = new MCPSettingsPage();
  const screenshots = path.resolve('reports/screenshots/mcp-save-offline');
  let endpoint: Server;
  let online = false;
  let original: Snapshot;
  let offlineSnapshot: Snapshot;
  let config: string;
  let initializeRequests = 0;
  let initializeDelayMs = 0;

  before(async () => {
    if (process.env.OPENBITFUN_E2E_STORAGE_GUARD !== '1') {
      throw new Error('Run this spec with isolated E2E user/home roots and the storage guard');
    }
    mkdirSync(screenshots, { recursive: true });
    endpoint = createServer(async (request, response) => {
      if (!online) {
        // The real MCP transport sees a broken TCP connection, without
        // changing the controller network or mocking the product API.
        request.socket.destroy();
        return;
      }
      if (request.method !== 'POST') {
        response.writeHead(405).end();
        return;
      }
      let body = '';
      for await (const chunk of request) body += chunk;
      const message = JSON.parse(body);
      if (message.id === undefined) {
        response.writeHead(202).end();
        return;
      }
      let result: unknown = {};
      if (message.method === 'initialize') {
        initializeRequests += 1;
        if (initializeDelayMs) await new Promise(resolve => setTimeout(resolve, initializeDelayMs));
        result = {
          protocolVersion: message.params.protocolVersion,
          capabilities: { tools: {} },
          serverInfo: { name: 'MCP network recovery fixture', version: '1.0.0' },
        };
      } else if (message.method === 'tools/list') result = { tools: [] };
      else if (message.method === 'resources/list') result = { resources: [] };
      else if (message.method === 'prompts/list') result = { prompts: [] };
      response.writeHead(200, { 'content-type': 'application/json' });
      response.end(JSON.stringify({ jsonrpc: '2.0', id: message.id, result }));
    });
    await new Promise<void>(resolve => endpoint.listen(0, '127.0.0.1', resolve));
    const address = endpoint.address();
    if (!address || typeof address === 'string') throw new Error('No fixture port');
    original = await invoke<Snapshot>('load_mcp_json_config');
    config = JSON.stringify({ mcpServers: {
      'e2e-network-recovery': {
        name: 'E2E Network Recovery MCP', type: 'streamable-http',
        url: `http://127.0.0.1:${address.port}/mcp`, enabled: true, autoStart: true,
      },
    } }, null, 2);
    await page.open();
  });

  it('reports saved with a warning, persists the config, and clears the draft while offline', async () => {
    await page.openEditor();
    await page.edit(config);
    await page.save();
    await page.warning.waitForDisplayed({ timeout: 45000 });
    const warning = await page.warning.getText();
    expect(warning).toMatch(/配置已保存|設定已儲存|configuration saved/i);
    expect(warning).toMatch(/自动重试|自動重試|retry connection failures automatically/i);
    expect(await $('.notification-item--error').isExisting()).toBe(false);
    await page.input.waitForDisplayed({ reverse: true });
    await page.screenshot(path.join(screenshots, '01-offline-saved-warning.png'));

    offlineSnapshot = await invoke<Snapshot>('load_mcp_json_config');
    expect(JSON.parse(offlineSnapshot.jsonConfig)).toEqual(JSON.parse(config));
    expect(offlineSnapshot.fingerprint).not.toBe(original.fingerprint);
    const appConfigPath = path.join(process.env.OPENBITFUN_E2E_USER_ROOT!, 'config', 'app.json');
    await browser.waitUntil(() => {
      const disk = JSON.parse(readFileSync(appConfigPath, 'utf8'));
      return JSON.stringify(disk.mcp_servers) === JSON.stringify(JSON.parse(offlineSnapshot.jsonConfig));
    }, { timeout: 10000, timeoutMsg: 'Saved MCP configuration was not written to app.json' });
    await page.openEditor();
    expect(JSON.parse(await page.input.getValue())).toEqual(JSON.parse(config));
    expect(await page.saveButton.isEnabled()).toBe(false);
    await page.screenshot(path.join(screenshots, '02-offline-persisted-config.png'));
    await page.closeEditor();
  });

  it('reconnects after network recovery without saving again or changing the persisted revision', async () => {
    online = true;
    await page.waitForConnected('e2e-network-recovery');
    expect(initializeRequests).toBeGreaterThan(0);
    const recovered = await invoke<Snapshot>('load_mcp_json_config');
    expect(recovered.fingerprint).toBe(offlineSnapshot.fingerprint);
    expect(recovered.jsonConfig).toBe(offlineSnapshot.jsonConfig);
    await page.dismissWarning();
    await page.screenshot(path.join(screenshots, '03-network-recovered.png'));
    await page.openEditor();
    const edited = JSON.parse(config);
    edited.mcpServers['e2e-network-recovery'].name = 'E2E Network Recovery MCP (updated)';
    await page.edit(JSON.stringify(edited, null, 2));
    await page.save();
    await page.input.waitForDisplayed({ reverse: true, timeout: 30000 });
    const savedAgain = await invoke<Snapshot>('load_mcp_json_config');
    expect(JSON.parse(savedAgain.jsonConfig)).toEqual(edited);
    await page.waitForConnected('e2e-network-recovery');
    expect(await $('.notification-item--error').isExisting()).toBe(false);
  });

  it('confirms persistence when connection setup exceeds the frontend request deadline', async () => {
    initializeDelayMs = 35000;
    await page.openEditor();
    const slowConfig = JSON.parse(config);
    slowConfig.mcpServers['e2e-network-recovery'].name = 'E2E Slow Network MCP';
    await page.edit(JSON.stringify(slowConfig, null, 2));
    const startedAt = Date.now();
    await page.save();
    await page.warning.waitForDisplayed({ timeout: 45000 });
    expect(Date.now() - startedAt).toBeGreaterThanOrEqual(29000);
    expect(await page.warning.getText()).toMatch(/配置已保存|設定已儲存|configuration saved/i);
    expect(await $('.notification-item--error').isExisting()).toBe(false);
    const persisted = await invoke<Snapshot>('load_mcp_json_config');
    expect(JSON.parse(persisted.jsonConfig)).toEqual(slowConfig);
    await page.input.waitForDisplayed({ reverse: true });
    await page.waitForConnected('e2e-network-recovery');
    await page.openEditor();
    expect(await page.saveButton.isEnabled()).toBe(false);
    expect(JSON.parse(await page.input.getValue())).toEqual(slowConfig);
    await page.screenshot(path.join(screenshots, '04-slow-network-saved.png'));
    await page.closeEditor();
  });

  after(async () => {
    try {
      if (original) {
        const current = await invoke<Snapshot>('load_mcp_json_config');
        await invoke('save_mcp_json_config', {
          jsonConfig: original.jsonConfig, expectedFingerprint: current.fingerprint,
        });
      }
    } finally {
      endpoint?.closeAllConnections();
      if (endpoint?.listening) await new Promise<void>(resolve => endpoint.close(() => resolve()));
    }
  });
});
