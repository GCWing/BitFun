import { $, browser, expect } from '@wdio/globals';
import { MCPSettingsPage } from '../page-objects/MCPSettingsPage';

type Snapshot = { jsonConfig: string; fingerprint: string };
async function invoke<T>(command: string, args: unknown = {}): Promise<T> {
  return browser.execute(async (name: string, params: unknown) => {
    const host = window as typeof window & { __TAURI__: { core: { invoke: (name: string, params: unknown) => Promise<T> } } };
    return host.__TAURI__.core.invoke(name, params);
  }, command, args);
}

// Live opt-in: exercises the documented public endpoint without account credentials.
const live = process.env.OPENBITFUN_E2E_HUAWEI_MCP === '1' ? describe : describe.skip;
live('L1 Huawei knowledge MCP health and document search', () => {
  const page = new MCPSettingsPage();
  const serverId = 'e2e-huawei-knowledge';
  let original: Snapshot;
  before(async () => {
    if (process.env.OPENBITFUN_E2E_STORAGE_GUARD !== '1') throw new Error('Isolated E2E storage required');
    original = await invoke<Snapshot>('load_mcp_json_config');
    await page.open();
    await page.openEditor();
    await page.edit(JSON.stringify({ mcpServers: { [serverId]: {
      type: 'http', url: 'https://connect-api.cloud.huawei.com/api/developerknowledge/mcp',
      enabled: true, autoStart: true,
    } } }));
    await page.save();
    await page.input.waitForDisplayed({ reverse: true, timeout: 45000 });
  });
  it('stays usable after the immediate health probe and calls searchDocuments', async () => {
    const row = await $(`[data-testid="mcp-server-item"][data-server-id="${serverId}"]`);
    await row.waitForDisplayed();
    // Check after the immediate heartbeat, not just the initial handshake.
    await browser.pause(3000);
    const servers = await invoke<Array<{ id: string; status: string; last_error?: string }>>('get_mcp_servers');
    console.log('HUAWEI_MCP_STATUS', JSON.stringify(servers.find(server => server.id === serverId)));
    expect(servers.find(server => server.id === serverId)?.status).toMatch(/^(healthy|connected)$/i);
    await page.waitForConnected(serverId);
    const result = await invoke<{ result: { content: Array<{ type: string; text?: string }>; isError: boolean } }>('send_mcp_app_message', { request: {
      serverId,
      jsonrpc: '2.0', id: 1, method: 'tools/call', params: {
        name: 'searchDocuments', arguments: { SearchDocumentsReq: { query: 'ArkUI Text component' } },
      },
    } });
    console.log('HUAWEI_MCP_SEARCH', JSON.stringify(result).slice(0, 1800));
    expect(result.result.isError).toBe(false);
    const text = result.result.content.find(item => item.type === 'text')?.text;
    const search = JSON.parse(text!);
    expect(search.code).toBe(0);
    expect(search.resultList.length).toBeGreaterThan(0);
    const document = await invoke<{ result: { content: Array<{ type: string; text?: string }>; isError: boolean } }>('send_mcp_app_message', { request: {
      serverId, jsonrpc: '2.0', id: 2, method: 'tools/call', params: {
        name: 'getDocumentsById', arguments: { GetDocumentsByIdRequest: { names: [search.resultList[0].parent] } },
      },
    } });
    expect(document.result.isError).toBe(false);
    const detail = JSON.parse(document.result.content.find(item => item.type === 'text')?.text!);
    expect(detail.code).toBe(0);
    expect(detail.resultList.length).toBeGreaterThan(0);
    console.log('HUAWEI_MCP_DOCUMENT', JSON.stringify(detail).slice(0, 800));
    await browser.pause(31000);
    const settled = await invoke<Array<{ id: string; status: string }>>('get_mcp_servers');
    expect(settled.find(server => server.id === serverId)?.status).toMatch(/^(healthy|connected)$/i);
    await browser.saveScreenshot('/tmp/huawei-mcp-e2e.png');
  });
  after(async () => {
    if (!original) return;
    const current = await invoke<Snapshot>('load_mcp_json_config');
    await invoke('save_mcp_json_config', { jsonConfig: original.jsonConfig, expectedFingerprint: current.fingerprint });
  });
});
