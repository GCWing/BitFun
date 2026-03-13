/**
 * L1 mermaid export spec: validates export notifications and completion flow.
 */

import { browser, expect, $ } from '@wdio/globals';
import { Header } from '../page-objects/components/Header';
import { StartupPage } from '../page-objects/StartupPage';
import { ensureWorkspaceOpen } from '../helpers/workspace-utils';
import { saveStepScreenshot } from '../helpers/screenshot-utils';

const SAMPLE_MERMAID = `flowchart TD
  A[Start] --> B{Check}
  B -->|Yes| C[Done]
  B -->|No| D[Retry]`;

describe('L1 Mermaid Export', () => {
  let header: Header;
  let startupPage: StartupPage;
  let hasWorkspace = false;

  const openMermaidEditor = async () => {
    await browser.execute((code: string) => {
      window.dispatchEvent(new CustomEvent('expand-right-panel'));
      window.dispatchEvent(new CustomEvent('agent-create-tab', {
        detail: {
          type: 'mermaid-editor',
          title: 'Mermaid Editor',
          data: {
            mermaid_code: code,
            sourceCode: code,
            mode: 'editor',
            allow_mode_switch: true,
            editor_config: {
              readonly: false,
              show_preview: true,
              auto_format: false,
            },
          },
          metadata: {
            duplicateCheckKey: 'e2e-mermaid-export',
            fromE2E: true,
          },
          checkDuplicate: true,
          duplicateCheckKey: 'e2e-mermaid-export',
          replaceExisting: true,
        },
      }));
    }, SAMPLE_MERMAID);

    await browser.waitUntil(async () => (await $('[data-testid="mermaid-editor"]')).isExisting(), {
      timeout: 15000,
      timeoutMsg: 'Mermaid editor did not open',
    });

    await browser.waitUntil(async () => {
      const svg = await $('.mermaid-preview svg');
      return svg.isExisting();
    }, {
      timeout: 15000,
      timeoutMsg: 'Mermaid preview SVG did not render',
    });
  };

  const openNotificationCenter = async () => {
    const button = await $('[data-testid="notification-button"]');
    await browser.waitUntil(async () => button.isExisting(), {
      timeout: 5000,
      timeoutMsg: 'Notification button not found',
    });
    await button.click();
    await browser.waitUntil(async () => (await $('[data-testid="notification-center"]')).isExisting(), {
      timeout: 5000,
      timeoutMsg: 'Notification center did not open',
    });
  };

  before(async () => {
    console.log('[L1] Starting mermaid export tests');
    header = new Header();
    startupPage = new StartupPage();

    await browser.pause(3000);
    await header.waitForLoad();
    hasWorkspace = await ensureWorkspaceOpen(startupPage);
  });

  beforeEach(async function () {
    if (!hasWorkspace) {
      this.skip();
      return;
    }

    await browser.execute(() => {
      (window as Window & { __BITFUN_E2E_PNG_EXPORT_DELAY_MS__?: number }).__BITFUN_E2E_PNG_EXPORT_DELAY_MS__ = 1200;
    });

    await openMermaidEditor();
  });

  afterEach(async () => {
    await browser.execute(() => {
      delete (window as Window & { __BITFUN_E2E_PNG_EXPORT_DELAY_MS__?: number }).__BITFUN_E2E_PNG_EXPORT_DELAY_MS__;
    });
  });

  it('should render the mermaid editor preview before export', async function () {
    if (!hasWorkspace) {
      this.skip();
      return;
    }

    const editor = await $('[data-testid="mermaid-editor"]');
    const previewSvg = await $('.mermaid-preview svg');

    expect(await editor.isExisting()).toBe(true);
    expect(await previewSvg.isExisting()).toBe(true);

    await saveStepScreenshot('l1-mermaid-export-preview-ready');
  });

  it('png export should show progress and finish successfully', async function () {
    if (!hasWorkspace) {
      this.skip();
      return;
    }

    await browser.execute(() => {
      const button = document.querySelector('[data-testid="mermaid-export-png"]') as HTMLButtonElement | null;
      button?.click();
    });

    await browser.waitUntil(async () => {
      const className = await (await $('[data-testid="notification-button"]')).getAttribute('class');
      return !!className && className.includes('bitfun-notification-btn--loading');
    }, {
      timeout: 5000,
      timeoutMsg: 'PNG export did not enter loading state',
    });

    await openNotificationCenter();

    const activeSection = await $('[data-testid="notification-center-active-section"]');
    expect(await activeSection.isExisting()).toBe(true);

    await saveStepScreenshot('l1-mermaid-export-loading');

    await browser.waitUntil(async () => {
      return browser.execute(() => {
        const button = document.querySelector('[data-testid="notification-button"]');
        return !(button?.classList.contains('bitfun-notification-btn--loading'));
      });
    }, {
      timeout: 20000,
      timeoutMsg: 'PNG export loading state did not finish',
    });

    await browser.waitUntil(async () => {
      return browser.execute(() => {
        const texts = Array.from(document.querySelectorAll('.notification-center'))
          .map(el => el.textContent || '')
          .join('\n');
        return /PNG/i.test(texts) && /(成功|success)/i.test(texts);
      });
    }, {
      timeout: 10000,
      timeoutMsg: 'PNG export success notification not found',
    });

    await saveStepScreenshot('l1-mermaid-export-success');
  });
});
