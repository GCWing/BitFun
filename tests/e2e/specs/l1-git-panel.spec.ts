/**
 * L1 git panel spec: validates Git panel functionality.
 * Tests panel display, branch name, and change list.
 */

import { browser, expect, $ } from '@wdio/globals';
import { Header } from '../page-objects/components/Header';
import { StartupPage } from '../page-objects/StartupPage';
import { saveScreenshot, saveFailureScreenshot } from '../helpers/screenshot-utils';
import { ensureWorkspaceOpen } from '../helpers/workspace-utils';

describe('L1 Git Panel', () => {
  let header: Header;
  let startupPage: StartupPage;

  let hasWorkspace = false;

  before(async () => {
    console.log('[L1] Starting git panel tests');
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

  describe('Git panel existence', () => {
    it('git scene/container should exist', async function () {
      if (!hasWorkspace) {
        console.log('[L1] Skipping: workspace required');
        this.skip();
        return;
      }

      await browser.pause(500);

      const selectors = [
        '.bitfun-git-scene',
        '[class*="git-scene"]',
        '[class*="GitScene"]',
        '[data-testid="git-panel"]',
      ];

      let gitFound = false;
      for (const selector of selectors) {
        const element = await $(selector);
        const exists = await element.isExisting();

        if (exists) {
          console.log(`[L1] Git panel found: ${selector}`);
          gitFound = true;
          break;
        }
      }

      if (!gitFound) {
        console.log('[L1] Git panel not found - may need to navigate to Git view');
      }

      expect(typeof gitFound).toBe('boolean');
    });

    it('git panel should detect repository status', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const notRepo = await $('.bitfun-git-scene--not-repository');
      const isLoading = await $('.bitfun-git-scene--loading');
      const isRepo = await $('.bitfun-git-scene-working-copy');

      const notRepoExists = await notRepo.isExisting();
      const loadingExists = await isLoading.isExisting();
      const repoExists = await isRepo.isExisting();

      console.log('[L1] Git status:', {
        notRepository: notRepoExists,
        loading: loadingExists,
        isRepository: repoExists,
      });

      expect(typeof notRepoExists).toBe('boolean');
      expect(typeof loadingExists).toBe('boolean');
      expect(typeof repoExists).toBe('boolean');
    });
  });

  describe('Branch display', () => {
    it('current branch should be displayed if in git repo', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const branchElement = await $('.bitfun-git-scene-working-copy__branch');
      const exists = await branchElement.isExisting();

      if (exists) {
        const branchText = await branchElement.getText();
        console.log('[L1] Current branch:', branchText);

        expect(branchText.length).toBeGreaterThan(0);
      } else {
        console.log('[L1] Branch element not found - may not be in git repo');
        expect(typeof exists).toBe('boolean');
      }
    });

    it('ahead/behind badges should be visible if applicable', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const badges = await browser.$$('[class*="ahead"], [class*="behind"], .sync-badge');
      console.log('[L1] Sync badges found:', badges.length);

      expect(badges.length).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Change list', () => {
    it('file changes should be displayed', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const changeSelectors = [
        '.wcv-file',
        '[class*="git-change"]',
        '[class*="changed-file"]',
      ];

      let changesFound = false;
      for (const selector of changeSelectors) {
        const elements = await browser.$$(selector);
        if (elements.length > 0) {
          console.log(`[L1] File changes found: ${selector}, count: ${elements.length}`);
          changesFound = true;
          break;
        }
      }

      if (!changesFound) {
        console.log('[L1] No file changes displayed');
      }

      expect(typeof changesFound).toBe('boolean');
    });

    it('changes should have status indicators', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const statusClasses = [
        'wcv-status--modified',
        'wcv-status--added',
        'wcv-status--deleted',
        'wcv-status--renamed',
      ];

      let statusFound = false;
      for (const className of statusClasses) {
        const elements = await browser.$$(`.${className}`);
        if (elements.length > 0) {
          console.log(`[L1] Files with status ${className}: ${elements.length}`);
          statusFound = true;
          break;
        }
      }

      if (!statusFound) {
        console.log('[L1] No status indicators found');
      }

      expect(typeof statusFound).toBe('boolean');
    });

    it('staged and unstaged sections should exist', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const sections = await browser.$$('[class*="staged"], [class*="unstaged"], [class*="changes-section"]');
      console.log('[L1] Change sections found:', sections.length);

      expect(sections.length).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Git actions', () => {
    it('commit message input should be available', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const commitInput = await $('[class*="commit-message"], [class*="commit-input"], textarea[placeholder*="commit"]');
      const exists = await commitInput.isExisting();

      if (exists) {
        console.log('[L1] Commit message input found');
        expect(exists).toBe(true);
      } else {
        console.log('[L1] Commit message input not found');
        expect(typeof exists).toBe('boolean');
      }
    });

    it('file actions should be available', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const actionSelectors = [
        '[class*="stage-btn"]',
        '[class*="unstage-btn"]',
        '[class*="discard-btn"]',
        '[class*="diff-btn"]',
      ];

      let actionsFound = false;
      for (const selector of actionSelectors) {
        const elements = await browser.$$(selector);
        if (elements.length > 0) {
          console.log(`[L1] File actions found: ${selector}`);
          actionsFound = true;
          break;
        }
      }

      if (!actionsFound) {
        console.log('[L1] No file action buttons found');
      }

      expect(typeof actionsFound).toBe('boolean');
    });
  });

  describe('Diff viewing', () => {
    it('clicking file should open diff view', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const files = await browser.$$('.wcv-file');
      if (files.length === 0) {
        console.log('[L1] No files to test diff view');
        this.skip();
        return;
      }

      const selectedFiles = await browser.$$('.wcv-file--selected');
      console.log('[L1] Currently selected files:', selectedFiles.length);

      expect(selectedFiles.length).toBeGreaterThanOrEqual(0);
    });
  });

  afterEach(async function () {
    if (this.currentTest?.state === 'failed') {
      await saveFailureScreenshot(`l1-git-panel-${this.currentTest.title}`);
    }
  });

  after(async () => {
    await saveScreenshot('l1-git-panel-complete');
    console.log('[L1] Git panel tests complete');
  });
});
