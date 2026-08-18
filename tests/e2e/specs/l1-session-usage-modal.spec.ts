/**
 * L1 desktop E2E coverage for the focused session-usage summary.
 *
 * The report payload is deterministic so the screenshot can be compared with
 * the product reference while the rendered surface, focus handling, clipboard
 * action, and details handoff all run inside the real Tauri WebView.
 */

import { browser, expect, $ } from '@wdio/globals';
import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';
import { dirname } from 'path';
import { expectPopupCloseContract } from '../helpers/popup-close-contract';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const report = {
  schemaVersion: 1,
  reportId: 'usage-e2e-reference',
  sessionId: 'session-usage-e2e',
  generatedAt: Date.UTC(2026, 7, 14, 6, 1),
  workspace: { kind: 'local', pathLabel: '[已脱敏路径]' },
  scope: { kind: 'entire_session', turnCount: 4, includesSubagents: false },
  coverage: {
    level: 'partial',
    available: ['workspace_identity', 'token_usage_records'],
    missing: ['token_detail_breakdown'],
    notes: [],
  },
  time: {
    accounting: 'approximate',
    denominator: 'session_wall_time',
    wallTimeMs: 820_000,
    activeTurnMs: 588_000,
  },
  tokens: {
    source: 'token_usage_records',
    inputTokens: 5_126_217,
    outputTokens: 270_000,
    totalTokens: 5_396_217,
    cachedTokens: 5_274_240,
    cacheCoverage: 'available',
    cacheHitRate: 0.99,
  },
  models: [{
    modelId: 'deepseek-v4-flash',
    callCount: 68,
    inputTokens: 5_126_217,
    outputTokens: 270_000,
    totalTokens: 5_396_217,
  }],
  tools: [
    { toolName: 'ExecCommand', category: 'shell', callCount: 9, successCount: 9, errorCount: 0, durationMs: 9000, redacted: false },
    { toolName: 'Grep', category: 'file', callCount: 19, successCount: 19, errorCount: 0, durationMs: 8000, redacted: false },
    { toolName: 'Edit', category: 'file', callCount: 19, successCount: 19, errorCount: 0, durationMs: 7000, redacted: false },
    { toolName: 'Read', category: 'file', callCount: 6, successCount: 6, errorCount: 0, durationMs: 6000, redacted: false },
    { toolName: 'Glob', category: 'file', callCount: 5, successCount: 5, errorCount: 0, durationMs: 5000, redacted: false },
    { toolName: 'Git', category: 'git', callCount: 4, successCount: 4, errorCount: 0, durationMs: 4000, redacted: false },
    { toolName: 'WebSearch', category: 'other', callCount: 3, successCount: 3, errorCount: 0, durationMs: 3000, redacted: false },
    { toolName: 'Task', category: 'other', callCount: 2, successCount: 2, errorCount: 0, durationMs: 2000, redacted: false },
  ],
  files: {
    scope: 'snapshot_summary',
    changedFiles: 1,
    files: [{ pathLabel: 'src/example.ts', operationCount: 1, redacted: false }],
  },
  compression: {
    compactionCount: 0,
    manualCompactionCount: 0,
    automaticCompactionCount: 0,
  },
  errors: { totalErrors: 3, toolErrors: 3, modelErrors: 0, examples: [] },
  slowest: [],
  privacy: {
    promptContentIncluded: false,
    toolInputsIncluded: false,
    commandOutputsIncluded: false,
    fileContentsIncluded: false,
    redactedFields: [],
  },
};

async function showUsageModal(): Promise<void> {
  await browser.execute(async (fixture) => {
    const [{ appearanceService }, { i18nService }, modalState] = await Promise.all([
      // @ts-expect-error Vite resolves browser-root modules inside the desktop WebView.
      import('/src/infrastructure/appearance/index.ts'),
      // @ts-expect-error Vite resolves browser-root modules inside the desktop WebView.
      import('/src/infrastructure/i18n/index.ts'),
      // @ts-expect-error Vite resolves browser-root modules inside the desktop WebView.
      import('/src/flow_chat/components/usage/sessionUsageModalState.ts'),
    ]);

    await appearanceService.activate('bitfun-light');
    await i18nService.changeLanguage('zh-CN');
    modalState.openSessionUsageModal({
      sessionId: fixture.sessionId,
      workspacePath: 'D:/workspace/BitFun',
    });
    modalState.showSessionUsageModalReport({
      sessionId: fixture.sessionId,
      report: fixture,
      markdown: '# Session Usage Report',
    });
  }, report);

  const modal = await $('[data-testid="session-usage-modal"]');
  await modal.waitForDisplayed({ timeout: 15000 });
  await browser.execute(() => {
    document
      .querySelectorAll<HTMLButtonElement>('.notification-item__close, .announcement-toast__close')
      .forEach(button => button.click());
  });
  await browser.waitUntil(async () => browser.execute(() => (
    !Array.from(document.querySelectorAll('.notification-item, .announcement-toast'))
      .some(element => element instanceof HTMLElement && element.offsetParent !== null)
  )), {
    timeout: 3000,
    interval: 100,
    timeoutMsg: 'Unrelated startup notices remained visible over the usage visual fixture',
  });
  await browser.executeAsync((done) => {
    requestAnimationFrame(() => requestAnimationFrame(() => done()));
  });
}

async function closeUsageModal(): Promise<void> {
  await browser.execute(async () => {
    // @ts-expect-error Vite resolves browser-root modules inside the desktop WebView.
    const { closeSessionUsageModal } = await import('/src/flow_chat/components/usage/sessionUsageModalState.ts');
    closeSessionUsageModal();
  }).catch(() => undefined);
}

describe('L1 Session usage modal', () => {
  before(async () => {
    await showUsageModal();
  });

  after(async () => {
    await closeUsageModal();
  });

  it('renders the focused summary without the partial-data badge', async () => {
    const modal = await $('[data-testid="session-usage-modal"]');
    const text = await modal.getText();
    const toolRows = await modal.$$('.session-usage-report-card__compact-tool-row');

    expect(text).toContain('会话用量');
    expect(text).toContain('5,396,217');
    expect(text).toContain('5,274,240 (99%)');
    expect(text).toContain('13分40秒');
    expect(text).toContain('9分48秒');
    expect(text).toContain('查看全部 8 项');
    expect(text).not.toContain('部分数据可用');
    expect(toolRows).toHaveLength(3);
    await expectPopupCloseContract('[data-testid="session-usage-modal"]');
  });

  it('captures the settled desktop WebView state for visual comparison', async () => {
    await browser.execute(() => {
      const modal = document.querySelector<HTMLElement>('[data-testid="session-usage-modal"]');
      modal?.focus({ preventScroll: true });
    });
    await browser.pause(350);

    const screenshotsDir = path.resolve(__dirname, '..', 'reports', 'screenshots');
    fs.mkdirSync(screenshotsDir, { recursive: true });
    const screenshotPath = path.join(screenshotsDir, 'session-usage-modal-e2e.png');
    const modalScreenshotPath = path.join(screenshotsDir, 'session-usage-modal-dialog-e2e.png');
    const metricsPath = path.join(screenshotsDir, 'session-usage-modal-e2e-metrics.json');
    await browser.saveScreenshot(screenshotPath);
    const modal = await $('[data-testid="session-usage-modal"]');
    await modal.saveScreenshot(modalScreenshotPath);
    const metrics = await browser.execute(() => {
      const overlay = document.querySelector<HTMLElement>('.session-usage-modal');
      const dialog = document.querySelector<HTMLElement>('[data-testid="session-usage-modal"]');
      const header = dialog?.querySelector<HTMLElement>('.modal__header');
      const title = dialog?.querySelector<HTMLElement>('.modal__title');
      const content = dialog?.querySelector<HTMLElement>('.modal__content');
      const dialogRect = dialog?.getBoundingClientRect();
      const style = (element: HTMLElement | null | undefined) => (
        element ? getComputedStyle(element) : null
      );
      return {
        viewport: { width: window.innerWidth, height: window.innerHeight, dpr: window.devicePixelRatio },
        dialogRect: dialogRect ? {
          x: dialogRect.x,
          y: dialogRect.y,
          width: dialogRect.width,
          height: dialogRect.height,
        } : null,
        overlayBackground: style(overlay)?.backgroundColor,
        dialogBackground: style(dialog)?.backgroundColor,
        dialogBorderRadius: style(dialog)?.borderRadius,
        dialogShadow: style(dialog)?.boxShadow,
        headerPadding: style(header)?.padding,
        headerBorderBottom: style(header)?.borderBottom,
        titleFontSize: style(title)?.fontSize,
        titleFontWeight: style(title)?.fontWeight,
        contentPadding: style(content)?.padding,
      };
    });
    fs.writeFileSync(metricsPath, `${JSON.stringify(metrics, null, 2)}\n`);
    console.log(`[SessionUsageE2E] ${JSON.stringify(metrics)}`);

    expect(fs.existsSync(screenshotPath)).toBe(true);
    expect(fs.existsSync(modalScreenshotPath)).toBe(true);
  });

  it('copies the report and hands off to the details tab', async () => {
    const copyButton = await $('[data-testid="session-usage-copy"]');
    await copyButton.click();
    await browser.waitUntil(async () => (
      await copyButton.getAttribute('aria-label')
    ) === '已复制', {
      timeout: 5000,
      interval: 100,
      timeoutMsg: 'Usage report copy action did not enter its copied state',
    });

    await browser.execute(() => {
      const testWindow = window as Window & {
        __sessionUsageCreatedTabs?: Array<{ type?: string; data?: { report?: { reportId?: string } } }>;
      };
      testWindow.__sessionUsageCreatedTabs = [];
      window.addEventListener('agent-create-tab', (event) => {
        testWindow.__sessionUsageCreatedTabs?.push(
          (event as CustomEvent<{ type?: string; data?: { report?: { reportId?: string } } }>).detail,
        );
      }, { once: true });
    });

    const detailsButton = await $('[data-testid="session-usage-details"]');
    await detailsButton.click();

    await browser.waitUntil(async () => {
      const modal = await $('[data-testid="session-usage-modal"]');
      return !(await modal.isDisplayed().catch(() => false));
    }, {
      timeout: 5000,
      interval: 100,
      timeoutMsg: 'Usage summary modal did not close after opening details',
    });

    await browser.waitUntil(async () => {
      return browser.execute(() => {
        const testWindow = window as Window & {
          __sessionUsageCreatedTabs?: Array<{ type?: string }>;
        };
        return testWindow.__sessionUsageCreatedTabs?.some(tab => tab.type === 'session-usage') ?? false;
      });
    }, {
      timeout: 5000,
      interval: 100,
      timeoutMsg: 'Usage details action did not create the session-usage tab',
    });

    const createdTabs = await browser.execute(() => {
      const testWindow = window as Window & {
        __sessionUsageCreatedTabs?: Array<{ type?: string; data?: { report?: { reportId?: string } } }>;
      };
      return testWindow.__sessionUsageCreatedTabs ?? [];
    });
    expect(createdTabs).toEqual(expect.arrayContaining([
      expect.objectContaining({
        type: 'session-usage',
        data: expect.objectContaining({
          report: expect.objectContaining({ reportId: 'usage-e2e-reference' }),
        }),
      }),
    ]));
  });
});
