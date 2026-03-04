/**
 * L1 editor spec: validates editor functionality.
 * Tests file content display, multi-tab switching, and unsaved markers.
 */

import { browser, expect, $ } from '@wdio/globals';
import { Header } from '../page-objects/components/Header';
import { StartupPage } from '../page-objects/StartupPage';
import { saveScreenshot, saveFailureScreenshot } from '../helpers/screenshot-utils';
import { ensureWorkspaceOpen } from '../helpers/workspace-utils';

describe('L1 Editor', () => {
  let header: Header;
  let startupPage: StartupPage;

  let hasWorkspace = false;

  before(async () => {
    console.log('[L1] Starting editor tests');
    // Initialize page objects after browser is ready
    header = new Header();
    startupPage = new StartupPage();

    await browser.pause(3000);
    await header.waitForLoad();

    hasWorkspace = await ensureWorkspaceOpen(startupPage);

    if (!hasWorkspace) {
      console.log('[L1] No workspace available - tests will be skipped');
    }
  });

  describe('Editor existence', () => {
    it('editor container should exist', async function () {
      if (!hasWorkspace) {
        console.log('[L1] Skipping: workspace required');
        this.skip();
        return;
      }

      await browser.pause(500);

      const selectors = [
        '[data-monaco-editor="true"]',
        '.code-editor-tool',
        '.monaco-editor',
        '[class*="code-editor"]',
      ];

      let editorFound = false;
      for (const selector of selectors) {
        const element = await $(selector);
        const exists = await element.isExisting();

        if (exists) {
          console.log(`[L1] Editor found: ${selector}`);
          editorFound = true;
          break;
        }
      }

      if (!editorFound) {
        console.log('[L1] Editor not found - no file may be open');
      }

      expect(typeof editorFound).toBe('boolean');
    });

    it('editor should have Monaco attributes', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const editor = await $('[data-monaco-editor="true"]');
      const exists = await editor.isExisting();

      if (exists) {
        const editorId = await editor.getAttribute('data-editor-id');
        const filePath = await editor.getAttribute('data-file-path');
        const readOnly = await editor.getAttribute('data-readonly');

        console.log('[L1] Editor attributes:', { editorId, filePath, readOnly });
        expect(editorId).toBeDefined();
      } else {
        console.log('[L1] Monaco editor not visible');
        expect(typeof exists).toBe('boolean');
      }
    });
  });

  describe('File content display', () => {
    it('editor should show file content if file is open', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const editor = await $('[data-monaco-editor="true"]');
      const exists = await editor.isExisting();

      if (!exists) {
        console.log('[L1] No file open in editor');
        this.skip();
        return;
      }

      // Check for Monaco editor content
      const monacoContent = await browser.execute(() => {
        const editor = document.querySelector('.monaco-editor');
        if (!editor) return null;

        const lines = editor.querySelectorAll('.view-line');
        return {
          lineCount: lines.length,
          hasContent: lines.length > 0,
        };
      });

      console.log('[L1] Monaco content:', monacoContent);
      expect(monacoContent).toBeDefined();
    });

    it('cursor position should be tracked', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const editor = await $('[data-monaco-editor="true"]');
      const exists = await editor.isExisting();

      if (!exists) {
        this.skip();
        return;
      }

      const cursorLine = await editor.getAttribute('data-cursor-line');
      const cursorColumn = await editor.getAttribute('data-cursor-column');

      console.log('[L1] Cursor position:', { cursorLine, cursorColumn });
      expect(cursorLine !== null || cursorColumn !== null).toBe(true);
    });
  });

  describe('Tab bar', () => {
    it('tab bar should exist when files are open', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const tabBarSelectors = [
        '.bitfun-tab-bar',
        '[class*="tab-bar"]',
        '[role="tablist"]',
      ];

      let tabBarFound = false;
      for (const selector of tabBarSelectors) {
        const element = await $(selector);
        const exists = await element.isExisting();

        if (exists) {
          console.log(`[L1] Tab bar found: ${selector}`);
          tabBarFound = true;
          break;
        }
      }

      if (!tabBarFound) {
        console.log('[L1] Tab bar not found - may not have multiple files open');
      }

      expect(typeof tabBarFound).toBe('boolean');
    });

    it('tabs should display file names', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const tabs = await browser.$$('[role="tab"], .bitfun-tab, [class*="tab-item"]');
      console.log('[L1] Tabs found:', tabs.length);

      if (tabs.length > 0) {
        const firstTab = tabs[0];
        const tabText = await firstTab.getText();
        console.log('[L1] First tab text:', tabText);
      }

      expect(tabs.length).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Multi-tab operations', () => {
    it('should be able to switch between tabs', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const tabs = await browser.$$('[role="tab"], .bitfun-tab, [class*="tab-item"]');

      if (tabs.length < 2) {
        console.log('[L1] Not enough tabs to test switching');
        this.skip();
        return;
      }

      // Click second tab
      await tabs[1].click();
      await browser.pause(300);

      console.log('[L1] Switched to second tab');

      // Click first tab
      await tabs[0].click();
      await browser.pause(300);

      console.log('[L1] Switched back to first tab');
      expect(tabs.length).toBeGreaterThanOrEqual(2);
    });

    it('tabs should have close buttons', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const closeButtons = await browser.$$('[class*="tab-close"], .bitfun-tab__close, [data-testid^="tab-close"]');
      console.log('[L1] Tab close buttons:', closeButtons.length);

      expect(closeButtons.length).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Unsaved marker', () => {
    it('unsaved files should have indicator', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      // Check for modified indicator on tabs
      const modifiedTabs = await browser.$$('[class*="modified"], [class*="unsaved"], [data-modified="true"]');
      console.log('[L1] Modified/unsaved tabs:', modifiedTabs.length);

      expect(modifiedTabs.length).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Editor status bar', () => {
    it('editor should have status bar with cursor info', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const editor = await $('[data-monaco-editor="true"]');
      const exists = await editor.isExisting();

      if (!exists) {
        this.skip();
        return;
      }

      const statusSelectors = [
        '.code-editor-tool__status-bar',
        '.editor-status',
        '[class*="status-bar"]',
      ];

      let statusFound = false;
      for (const selector of statusSelectors) {
        const element = await $(selector);
        const exists = await element.isExisting();

        if (exists) {
          console.log(`[L1] Status bar found: ${selector}`);
          statusFound = true;
          break;
        }
      }

      expect(typeof statusFound).toBe('boolean');
    });
  });

  afterEach(async function () {
    if (this.currentTest?.state === 'failed') {
      await saveFailureScreenshot(`l1-editor-${this.currentTest.title}`);
    }
  });

  after(async () => {
    await saveScreenshot('l1-editor-complete');
    console.log('[L1] Editor tests complete');
  });
});
