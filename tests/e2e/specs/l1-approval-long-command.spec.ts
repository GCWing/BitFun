import { $, browser, expect } from '@wdio/globals';
import { createServer, type Server } from 'node:http';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { ApprovalBandPage } from '../page-objects/ApprovalBandPage';
import { openWorkspaceThroughFrontend } from '../helpers/workspace-helper';

async function invoke<T>(command: string, request: Record<string, unknown>): Promise<T> {
  return browser.execute(async (name, payload) => {
    return window.__TAURI__!.core!.invoke!(name, { request: payload }) as Promise<T>;
  }, command, request);
}

// Only the model endpoint is deterministic. Sessions, tool execution, approval
// requests, UI events, replies, and audit storage use the real desktop runtime.
describe('L1 long-command approval', () => {
  const page = new ApprovalBandPage();
  const screenshots = path.resolve('reports/screenshots/approval-long-command');
  const workspace = mkdtempSync(path.join(tmpdir(), 'approval-ui-e2e-'));
  let endpoint: Server;
  let commands: string[] = [];
  let delivered = false;
  let modelResult = '';
  let sessionId = '';
  let workspaceId = '';
  const measurements: unknown[] = [];

  before(async () => {
    if (process.env.OPENBITFUN_E2E_STORAGE_GUARD !== '1') throw new Error('Isolated profile required');
    mkdirSync(screenshots, { recursive: true });
    endpoint = createServer(async (request, response) => {
      let body = '';
      for await (const chunk of request) body += chunk;
      const input = JSON.parse(body || '{}');
      const toolResults = (input.messages || []).filter((message: any) => message.role === 'tool');
      const tools = input.tools || [];
      const wireName = tools.map((tool: any) => tool.function?.name)
        .find((name: string) => /^(ExecCommand|Bash)$/.test(name));
      const delta: any = { role: 'assistant' };
      let finishReason = 'stop';
      if (!delivered && wireName) {
        delivered = true;
        delta.tool_calls = commands.map((command, index) => ({
          index, id: `call_approval_${index}_${Date.now()}`, type: 'function',
          function: { name: wireName, arguments: JSON.stringify(wireName === 'Bash'
            ? { command } : { cmd: command, workdir: workspace, yield_time_ms: 1000 }) },
        }));
        finishReason = 'tool_calls';
      } else {
        if (toolResults.length) modelResult = JSON.stringify(toolResults);
        delta.content = toolResults.length ? '审批流程已完成，执行结果已收到。' : '长命令审批测试';
      }
      response.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-cache' });
      const chunk = (value: unknown, reason: string | null) => JSON.stringify({
        id: 'chatcmpl-approval-e2e', object: 'chat.completion.chunk', created: 1,
        model: 'approval-e2e', choices: [{ index: 0, delta: value, finish_reason: reason }],
      });
      response.write(`data: ${chunk(delta, null)}\n\n`);
      response.end(`data: ${chunk({}, finishReason)}\n\ndata: [DONE]\n\n`);
    });
    await new Promise<void>(resolve => endpoint.listen(0, '127.0.0.1', resolve));
    const address = endpoint.address();
    if (!address || typeof address === 'string') throw new Error('Missing endpoint');
    await invoke('set_config', { path: 'ai.models', value: [{
      id: 'approval-e2e', name: 'Approval UI E2E', provider: 'openai', model_name: 'approval-e2e',
      base_url: `http://127.0.0.1:${address.port}/v1`, api_key: 'local-fixture',
      context_window: 128000, max_tokens: 4096, enabled: true,
      category: 'general_chat', capabilities: ['text_chat', 'function_calling'],
    }] });
    await invoke('set_config', { path: 'ai.default_models', value: { primary: 'approval-e2e', fast: 'approval-e2e' } });
    await openWorkspaceThroughFrontend(workspace);
    const current = await invoke<{ id: string }>('get_current_workspace', {});
    workspaceId = current.id;
  });

  async function viewport(width: number, height: number) {
    // The embedded macOS driver reports native pixels, while layout uses CSS pixels.
    const ratio = await browser.execute(() => devicePixelRatio);
    await browser.setWindowSize(Math.round(width * ratio), Math.round(height * ratio));
    await browser.waitUntil(async () => {
      const actual = await browser.execute(() => ({ width: innerWidth, height: innerHeight }));
      return Math.abs(actual.width - width) <= 2 && Math.abs(actual.height - height) <= 2;
    }, { timeout: 5000, timeoutMsg: 'Native window did not reach the requested CSS viewport' });
  }

  async function start(label: string, nextCommands: string[]) {
    commands = nextCommands;
    delivered = false;
    modelResult = '';
    sessionId = randomUUID();
    await invoke('create_session', {
      sessionId, sessionName: label, agentType: 'agentic', workspacePath: workspace,
      config: { modelName: 'approval-e2e' },
    });
    await browser.refresh();
    const item = await $(`[data-testid="nav-session-item"][data-session-id="${sessionId}"]`);
    await item.waitForDisplayed({ timeout: 30000 });
    await item.click();
    await invoke('start_dialog_turn', {
      sessionId, userInput: label, agentType: 'agentic', workspacePath: workspace,
    });
    await page.band.waitForDisplayed({ timeout: 45000 });
  }

  async function screenshot(name: string) {
    // Let the shared segmented-control selection transition settle before capture.
    await browser.pause(250);
    await browser.saveScreenshot(path.join(screenshots, `${name}.png`));
    await page.band.saveScreenshot(path.join(screenshots, `${name}-detail.png`));
  }

  async function assertReply(reply: string, count = 1) {
    await browser.waitUntil(async () => {
      const { records } = await invoke<{ records: any[] }>('list_project_permission_audit', { workspaceId });
      return records.filter(record => record.request.sessionId === sessionId
        && record.event.event === 'replied' && record.event.reply.reply === reply).length === count;
    }, { timeout: 15000, timeoutMsg: `Missing ${reply} audit record` });
    await page.band.waitForDisplayed({ reverse: true, timeout: 15000 });
    await browser.waitUntil(() => modelResult.length > 0, { timeout: 15000 });
  }

  it('keeps reject reachable with a long multiline command and records rejection', async () => {
    await viewport(1280, 800);
    const command = Array.from({ length: 16 }, (_, i) => `printf '%s\\n' 'approval-ui-line-${i}: Android / HarmonyOS build configuration and tracked files'`).join('\n');
    await start('检查长命令审批布局', [command]);
    measurements.push({ case: 'multiline-desktop', ...await page.assertContained() });
    expect(await page.resource.getText()).toBe(command);
    await screenshot('01-desktop-long-command');
    await page.reject.click();
    await assertReply('reject');
    expect(modelResult).toMatch(/reject|denied|拒绝/i);
  });

  it('keeps allow reachable with an unbroken command at a narrower window and executes it', async () => {
    await viewport(800, 700);
    await start('验证窄窗口允许执行', [`printf 'approval-e2e-executed'; # ${'long-command-'.repeat(2500)}`]);
    measurements.push({ case: 'unbroken-narrow', ...await page.assertContained() });
    await screenshot('02-narrow-long-command');
    await page.allow.click();
    await assertReply('once');
    expect(modelResult).toContain('approval-e2e-executed');
  });

  it('uses the shared scope control to reject all pending commands', async () => {
    await viewport(1000, 750);
    await start('验证批量审批', [
      `printf 'batch-one'; # ${'review-command '.repeat(200)}`,
      "printf 'batch-two'",
    ]);
    measurements.push({ case: 'batch', ...await page.assertContained() });
    const all = await $('[data-openbitfun-component="segmented-control"] [data-openbitfun-value="all"]');
    await all.click();
    expect(await all.getAttribute('aria-checked')).toBe('true');
    await screenshot('03-batch-approval');
    await page.reject.click();
    await assertReply('reject', 2);
  });

  after(async () => {
    writeFileSync(path.join(screenshots, 'layout-measurements.json'), JSON.stringify(measurements, null, 2));
    endpoint?.closeAllConnections();
    if (endpoint?.listening) await new Promise<void>(resolve => endpoint.close(() => resolve()));
  });
});
