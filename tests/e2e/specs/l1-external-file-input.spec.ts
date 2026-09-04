import { browser, expect } from '@wdio/globals';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Header } from '../page-objects/components/Header';
import { StartupPage } from '../page-objects/StartupPage';
import { ensureCodeSessionOpen, openWorkspace } from '../helpers/workspace-helper';

describe('L1 external file composer input', () => {
  let fixtureRoot = '';
  let hasWorkspace = false;

  before(async () => {
    const header = new Header();
    const startupPage = new StartupPage();

    fixtureRoot = mkdtempSync(join(tmpdir(), 'openbitfun-external-file-e2e-'));
    writeFileSync(join(fixtureRoot, 'notes.txt'), 'external file context\n', 'utf8');
    writeFileSync(join(fixtureRoot, 'preview.png'), Buffer.from([0x89, 0x50, 0x4e, 0x47]));
    mkdirSync(join(fixtureRoot, 'reference-folder'));

    await browser.pause(3000);
    await header.waitForLoad();
    hasWorkspace = !(await startupPage.isVisible());
    if (!hasWorkspace) hasWorkspace = await openWorkspace();
    if (hasWorkspace) {
      try {
        await ensureCodeSessionOpen();
      } catch (error) {
        console.error('[L1] Failed to ensure Code session is open:', error);
        const existingSessions = await $$('[data-testid="nav-session-item"]').getElements();
        if (existingSessions.length > 0) {
          await existingSessions[existingSessions.length - 1].click();
          await browser.pause(500);
        }
        const existingInput = await $('.rich-text-input[contenteditable="true"]');
        hasWorkspace = await existingInput.isExisting();
      }
    }
  });

  it('adds multiple clipboard file paths as file, directory, and image contexts', async function () {
    if (!hasWorkspace) {
      this.skip();
      return;
    }

    const paths = [
      join(fixtureRoot, 'notes.txt'),
      join(fixtureRoot, 'reference-folder'),
      join(fixtureRoot, 'preview.png'),
      join(fixtureRoot, 'notes.txt'),
    ];
    const editor = await $('.rich-text-input[contenteditable="true"]');
    await editor.waitForExist({ timeout: 5000 });

    const dispatchResult = await browser.execute(async (clipboardPaths) => {
      const apiModulePath = '/src/infrastructure/api/service-api/ApiClient.ts';
      const { apiClient } = await import(/* @vite-ignore */ apiModulePath);
      const adapter = apiClient.getAdapter() as unknown as {
        invokeFn: ((command: string, args?: unknown, options?: unknown) => Promise<unknown>) | null;
      };
      const originalInvoke = adapter.invokeFn;
      if (!originalInvoke) throw new Error('Tauri transport adapter is not initialized');

      const target = document.querySelector<HTMLElement>('.rich-text-input[contenteditable="true"]');
      const e2eWindow = window as unknown as {
        __externalFileE2eCommands?: string[];
        __restoreE2eInvoke?: () => void;
      };
      e2eWindow.__externalFileE2eCommands = [];
      e2eWindow.__restoreE2eInvoke = () => {
        adapter.invokeFn = originalInvoke;
      };
      adapter.invokeFn = (command, args, options) => {
        e2eWindow.__externalFileE2eCommands?.push(command);
        if (command === 'get_clipboard_files') {
          return Promise.resolve({ files: clipboardPaths, isCut: false });
        }
        return originalInvoke(command, args, options);
      };

      target?.focus();
      const event = new ClipboardEvent('paste', { bubbles: true, cancelable: true });
      Object.defineProperty(event, 'clipboardData', {
        value: {
          items: [{ kind: 'file', type: 'application/octet-stream', getAsFile: () => null }],
          types: ['Files'],
          getData: () => '',
        },
      });
      target?.dispatchEvent(event);
      return {
        targetFound: Boolean(target),
        defaultPrevented: event.defaultPrevented,
      };
    }, paths);

    try {
      expect(dispatchResult).toEqual({ targetFound: true, defaultPrevented: true });
      await browser.waitUntil(async () => {
        const tags = await $$('[data-openbitfun-part="contextTag"]').getElements();
        const images = await $$('[data-testid="chat-input-image-strip"] [data-openbitfun-part="image"]').getElements();
        return tags.length === 2 && images.length === 1;
      }, {
        timeout: 5000,
        interval: 100,
        timeoutMsg: 'External clipboard paths were not added to the composer',
      });

      const invokedCommands = await browser.execute(() => (
        (window as unknown as { __externalFileE2eCommands?: string[] }).__externalFileE2eCommands ?? []
      ));
      expect(invokedCommands.filter(command => (
        command === 'get_clipboard_files' || command === 'get_file_metadata'
      ))).toEqual([
        'get_clipboard_files',
        'get_file_metadata',
        'get_file_metadata',
        'get_file_metadata',
      ]);

      const tagTypes = await browser.execute(() => Array.from(
        document.querySelectorAll<HTMLElement>('[data-openbitfun-part="contextTag"]'),
        element => element.dataset.openbitfunContextType,
      ));
      const tagTitles = await browser.execute(() => Array.from(
        document.querySelectorAll<HTMLElement>('[data-openbitfun-part="contextTag"]'),
        element => element.title,
      ));
      const imageTitle = await $('[data-testid="chat-input-image-strip"] [data-openbitfun-part="image"]').getAttribute('title');

      expect(tagTypes).toEqual(['file', 'directory']);
      expect(tagTitles).toEqual([paths[0], `${paths[1]} (recursive)`]);
      expect(imageTitle).toBe('preview.png');
    } finally {
      await browser.execute(() => {
        (window as unknown as { __restoreE2eInvoke?: () => void }).__restoreE2eInvoke?.();
        delete (window as unknown as { __restoreE2eInvoke?: () => void }).__restoreE2eInvoke;
      });
    }
  });

  after(() => {
    if (fixtureRoot) rmSync(fixtureRoot, { recursive: true, force: true });
  });
});
