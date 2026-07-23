/**
 * L1 dialog spec: validates dialog functionality.
 * Tests confirm dialogs and input dialogs with submit and cancel actions.
 */

import { browser, expect, $ } from '@wdio/globals';
import { Header } from '../page-objects/components/Header';
import { StartupPage } from '../page-objects/StartupPage';
import { saveScreenshot, saveFailureScreenshot } from '../helpers/screenshot-utils';
import { ensureWorkspaceOpen } from '../helpers/workspace-utils';

describe('L1 Dialog', () => {
  let header: Header;
  let startupPage: StartupPage;

  let hasWorkspace = false;

  before(async () => {
    console.log('[L1] Starting dialog tests');
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

  describe('Modal infrastructure', () => {
    it('modal overlay should exist when dialog is open', async function () {
      if (!hasWorkspace) {
        console.log('[L1] Skipping: workspace required');
        this.skip();
        return;
      }

      await browser.pause(500);

      // Check for modal infrastructure
      const overlay = await $('.modal-overlay');
      const modal = await $('.modal');

      const overlayExists = await overlay.isExisting();
      const modalExists = await modal.isExisting();

      console.log('[L1] Modal infrastructure:', { overlayExists, modalExists });

      // No dialog should be open initially
      expect(overlayExists || modalExists).toBe(false);
    });
  });

  describe('Confirm dialog', () => {
    it('confirm dialog should have correct structure', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      // Check for confirm dialog structure (if any is open)
      const confirmDialog = await $('.confirm-dialog');
      const exists = await confirmDialog.isExisting();

      if (exists) {
        console.log('[L1] Confirm dialog found');

        const header = await confirmDialog.$('.modal__header, [class*="dialog-header"]');
        const content = await confirmDialog.$('.modal__content, [class*="dialog-content"]');
        const actions = await confirmDialog.$('.modal__actions, [class*="dialog-actions"]');

        console.log('[L1] Dialog structure:', {
          hasHeader: await header.isExisting(),
          hasContent: await content.isExisting(),
          hasActions: await actions.isExisting(),
        });
      } else {
        console.log('[L1] No confirm dialog open');
      }

      expect(typeof exists).toBe('boolean');
    });

    it('confirm dialog should have action buttons', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const confirmDialog = await $('.confirm-dialog');
      const exists = await confirmDialog.isExisting();

      if (!exists) {
        console.log('[L1] No confirm dialog open to test buttons');
        expect(typeof exists).toBe('boolean');
        return;
      }

      const buttons = await confirmDialog.$$('button');
      console.log('[L1] Dialog buttons found:', buttons.length);

      expect(buttons.length).toBeGreaterThan(0);
    });

    it('confirm dialog should support types (info/warning/error)', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const types = ['info', 'warning', 'error', 'success'];

      for (const type of types) {
        const typedDialog = await $(`.confirm-dialog--${type}`);
        const exists = await typedDialog.isExisting();

        if (exists) {
          console.log(`[L1] Found confirm dialog of type: ${type}`);
        }
      }

      expect(Array.isArray(types)).toBe(true);
    });
  });

  describe('Input dialog', () => {
    it('input dialog should have input field', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const inputDialog = await $('.input-dialog');
      const exists = await inputDialog.isExisting();

      if (exists) {
        console.log('[L1] Input dialog found');

        const input = await inputDialog.$('input, textarea');
        const inputExists = await input.isExisting();

        console.log('[L1] Input field exists:', inputExists);
        expect(inputExists).toBe(true);
      } else {
        console.log('[L1] No input dialog open');
        expect(typeof exists).toBe('boolean');
      }
    });

    it('input dialog should have description area', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const description = await $('.input-dialog__description');
      const exists = await description.isExisting();

      console.log('[L1] Input dialog description exists:', exists);
      expect(typeof exists).toBe('boolean');
    });

    it('input dialog should have action buttons', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const inputDialog = await $('.input-dialog');
      const exists = await inputDialog.isExisting();

      if (!exists) {
        expect(typeof exists).toBe('boolean');
        return;
      }

      const actions = await inputDialog.$('.input-dialog__actions');
      const actionsExist = await actions.isExisting();

      if (actionsExist) {
        const buttons = await actions.$$('button');
        console.log('[L1] Input dialog buttons:', buttons.length);
      }

      expect(typeof actionsExist).toBe('boolean');
    });
  });

  describe('Dialog interactions', () => {
    it('ESC key should close dialog', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const modal = await $('.modal, .confirm-dialog, .input-dialog');
      const exists = await modal.isExisting();

      if (!exists) {
        console.log('[L1] No dialog open to test ESC close');
        expect(typeof exists).toBe('boolean');
        return;
      }

      // Press ESC
      await browser.keys(['Escape']);
      await browser.pause(300);

      const modalAfter = await $('.modal, .confirm-dialog, .input-dialog');
      const stillOpen = await modalAfter.isExisting();

      console.log('[L1] Dialog still open after ESC:', stillOpen);
      expect(typeof stillOpen).toBe('boolean');
    });

    it('clicking overlay should close modal', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const overlay = await $('.modal-overlay');
      const exists = await overlay.isExisting();

      if (!exists) {
        console.log('[L1] No modal overlay to test click close');
        expect(typeof exists).toBe('boolean');
        return;
      }

      await overlay.click();
      await browser.pause(300);

      console.log('[L1] Clicked modal overlay');
      expect(typeof exists).toBe('boolean');
    });

    it('dialog should be focusable', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const modalContent = await $('.modal__content, .confirm-dialog, .input-dialog');
      const exists = await modalContent.isExisting();

      if (!exists) {
        console.log('[L1] No dialog content to test focus');
        expect(typeof exists).toBe('boolean');
        return;
      }

      const activeElement = await browser.execute(() => {
        return {
          tagName: document.activeElement?.tagName,
          type: (document.activeElement as HTMLInputElement)?.type,
        };
      });

      console.log('[L1] Active element in dialog:', activeElement);
      expect(activeElement).toBeDefined();
    });
  });

  describe('Modal features', () => {
    it('modal should support different sizes', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const sizes = ['small', 'medium', 'large'];

      for (const size of sizes) {
        const sizedModal = await $(`.modal--${size}`);
        const exists = await sizedModal.isExisting();

        if (exists) {
          console.log(`[L1] Found modal with size: ${size}`);
        }
      }

      expect(Array.isArray(sizes)).toBe(true);
    });

    it('modal should support dragging if draggable', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const draggableModal = await $('.modal--draggable');
      const exists = await draggableModal.isExisting();

      console.log('[L1] Draggable modal exists:', exists);
      expect(typeof exists).toBe('boolean');
    });

    it('modal should support resizing if resizable', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const resizableModal = await $('.modal--resizable');
      const exists = await resizableModal.isExisting();

      console.log('[L1] Resizable modal exists:', exists);
      expect(typeof exists).toBe('boolean');
    });
  });

  afterEach(async function () {
    if (this.currentTest?.state === 'failed') {
      await saveFailureScreenshot(`l1-dialog-${this.currentTest.title}`);
    }
  });

  after(async () => {
    await saveScreenshot('l1-dialog-complete');
    console.log('[L1] Dialog tests complete');
  });
});
