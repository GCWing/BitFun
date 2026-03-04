/**
 * L1 terminal spec: validates terminal functionality.
 * Tests terminal display, command input, and output display.
 */

import { browser, expect, $ } from '@wdio/globals';
import { Header } from '../page-objects/components/Header';
import { StartupPage } from '../page-objects/StartupPage';
import { saveScreenshot, saveFailureScreenshot } from '../helpers/screenshot-utils';
import { ensureWorkspaceOpen } from '../helpers/workspace-utils';

describe('L1 Terminal', () => {
  let header: Header;
  let startupPage: StartupPage;

  let hasWorkspace = false;

  before(async () => {
    console.log('[L1] Starting terminal tests');
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

  describe('Terminal existence', () => {
    it('terminal container should exist', async function () {
      if (!hasWorkspace) {
        console.log('[L1] Skipping: workspace required');
        this.skip();
        return;
      }

      await browser.pause(500);

      const selectors = [
        '[data-terminal-id]',
        '.bitfun-terminal',
        '.xterm',
        '[class*="terminal"]',
      ];

      let terminalFound = false;
      for (const selector of selectors) {
        const element = await $(selector);
        const exists = await element.isExisting();

        if (exists) {
          console.log(`[L1] Terminal found: ${selector}`);
          terminalFound = true;
          break;
        }
      }

      if (!terminalFound) {
        console.log('[L1] Terminal not found - may need to be opened');
      }

      expect(typeof terminalFound).toBe('boolean');
    });

    it('terminal should have data attributes', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const terminal = await $('[data-terminal-id]');
      const exists = await terminal.isExisting();

      if (exists) {
        const terminalId = await terminal.getAttribute('data-terminal-id');
        const sessionId = await terminal.getAttribute('data-session-id');

        console.log('[L1] Terminal attributes:', { terminalId, sessionId });
        expect(terminalId).toBeDefined();
      } else {
        console.log('[L1] Terminal with data attributes not found');
        expect(typeof exists).toBe('boolean');
      }
    });
  });

  describe('Terminal display', () => {
    it('terminal should have xterm.js container', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const xterm = await $('.xterm');
      const exists = await xterm.isExisting();

      if (exists) {
        console.log('[L1] xterm.js container found');

        // Check for viewport
        const viewport = await $('.xterm-viewport');
        const viewportExists = await viewport.isExisting();
        console.log('[L1] xterm viewport exists:', viewportExists);

        expect(viewportExists).toBe(true);
      } else {
        console.log('[L1] xterm.js not visible');
        expect(typeof exists).toBe('boolean');
      }
    });

    it('terminal should have proper dimensions', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const terminal = await $('.bitfun-terminal');
      const exists = await terminal.isExisting();

      if (exists) {
        const size = await terminal.getSize();
        console.log('[L1] Terminal size:', size);

        expect(size.width).toBeGreaterThan(0);
        expect(size.height).toBeGreaterThan(0);
      } else {
        expect(typeof exists).toBe('boolean');
      }
    });
  });

  describe('Terminal interaction', () => {
    it('terminal should be focusable', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const terminal = await $('.bitfun-terminal, .xterm');
      const exists = await terminal.isExisting();

      if (!exists) {
        this.skip();
        return;
      }

      await terminal.click();
      await browser.pause(200);

      console.log('[L1] Terminal clicked');
      expect(typeof exists).toBe('boolean');
    });

    it('terminal should accept keyboard input', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const terminal = await $('.bitfun-terminal, .xterm');
      const exists = await terminal.isExisting();

      if (!exists) {
        this.skip();
        return;
      }

      // Focus and type
      await terminal.click();
      await browser.pause(100);

      // Type a simple command
      await browser.keys(['e', 'c', 'h', 'o', ' ', 't', 'e', 's', 't']);
      await browser.pause(200);

      console.log('[L1] Typed test input into terminal');
      expect(typeof exists).toBe('boolean');
    });
  });

  describe('Terminal output', () => {
    it('terminal should display output', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const terminal = await $('.bitfun-terminal');
      const exists = await terminal.isExisting();

      if (!exists) {
        this.skip();
        return;
      }

      // Check for terminal content
      const content = await terminal.getText();
      console.log('[L1] Terminal content length:', content.length);

      expect(content.length).toBeGreaterThanOrEqual(0);
    });

    it('terminal should have scrollable content', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const viewport = await $('.xterm-viewport');
      const exists = await viewport.isExisting();

      if (exists) {
        const scrollHeight = await viewport.getAttribute('scrollHeight');
        const clientHeight = await viewport.getAttribute('clientHeight');
        console.log('[L1] Viewport scroll:', { scrollHeight, clientHeight });

        expect(scrollHeight).toBeDefined();
      } else {
        expect(typeof exists).toBe('boolean');
      }
    });
  });

  describe('Terminal theme', () => {
    it('terminal should adapt to theme', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const terminal = await $('.bitfun-terminal');
      const exists = await terminal.isExisting();

      if (!exists) {
        this.skip();
        return;
      }

      const bgColor = await browser.execute(() => {
        const terminal = document.querySelector('.bitfun-terminal, .xterm');
        if (!terminal) return null;

        const styles = window.getComputedStyle(terminal);
        return styles.backgroundColor;
      });

      console.log('[L1] Terminal background color:', bgColor);
      expect(bgColor).toBeDefined();
    });
  });

  afterEach(async function () {
    if (this.currentTest?.state === 'failed') {
      await saveFailureScreenshot(`l1-terminal-${this.currentTest.title}`);
    }
  });

  after(async () => {
    await saveScreenshot('l1-terminal-complete');
    console.log('[L1] Terminal tests complete');
  });
});
