/**
 * L0 open workspace spec: verifies workspace opening flow.
 * Tests the ability to detect and interact with startup page and workspace state.
 */

import { browser, expect, $ } from '@wdio/globals';
import { getWorkspaceState, openWorkspace } from '../helpers/workspace-helper';
import { saveStepScreenshot } from '../helpers/screenshot-utils';

describe('L0 Workspace Opening', () => {
  let hasWorkspace = false;

  describe('App initialization', () => {
    it('app should start successfully', async () => {
      console.log('[L0] Waiting for app initialization...');
      await browser.pause(2000);
      const title = await browser.getTitle();
      console.log('[L0] App title:', title);
      expect(title).toBeDefined();
      await saveStepScreenshot('l0-workspace-app-started');
    });

    it('should have valid DOM structure', async () => {
      const body = await $('body');
      const html = await body.getHTML();
      expect(html.length).toBeGreaterThan(100);
      console.log('[L0] DOM loaded, HTML length:', html.length);
    });
  });

  describe('Workspace opening', () => {
    it('should open workspace successfully', async () => {
      await browser.pause(2000);

      hasWorkspace = await openWorkspace();

      console.log('[L0] Workspace opened:', hasWorkspace);
      expect(hasWorkspace).toBe(true);
      if (hasWorkspace) {
        await saveStepScreenshot('l0-workspace-opened');
      }
    });

    it('should have workspace UI elements', async () => {
      expect(hasWorkspace).toBe(true);

      const state = await getWorkspaceState();
      const navLabels = state.workspaceLabels.length;
      const sceneBar = await $('.bitfun-scene-bar');
      const hasSceneBar = await sceneBar.isExisting();

      console.log('[L0] Workspace state:', state);
      console.log('[L0] Scene bar exists:', hasSceneBar);
      expect(state.currentWorkspacePath).toBeTruthy();
      expect(navLabels).toBeGreaterThan(0);
      expect(hasSceneBar).toBe(true);
      await saveStepScreenshot('l0-workspace-chat-ready');
    });
  });

  describe('UI stability check', () => {
    it('UI should remain stable', async () => {
      expect(hasWorkspace).toBe(true);

      console.log('[L0] Monitoring UI stability for 10 seconds...');

      for (let i = 0; i < 2; i++) {
        await browser.pause(5000);

        const body = await $('body');
        const childCount = await body.$$('*').then(els => els.length);
        console.log(`[L0] ${(i + 1) * 5}s - DOM elements: ${childCount}`);

        expect(childCount).toBeGreaterThan(10);
      }

      console.log('[L0] UI stability confirmed');
    });
  });
});
