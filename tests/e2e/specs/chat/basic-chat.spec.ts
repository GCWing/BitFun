/**
 * Basic chat spec: chat input and core interaction.
 */

import { browser, expect, $ } from '@wdio/globals';
import { ChatPage } from '../../page-objects/ChatPage';
import { ChatInput } from '../../page-objects/components/ChatInput';
import { Header } from '../../page-objects/components/Header';
import { StartupPage } from '../../page-objects/StartupPage';
import { saveScreenshot, saveFailureScreenshot } from '../../helpers/screenshot-utils';
import { waitForElementStable } from '../../helpers/wait-utils';

describe('BitFun basic chat', () => {
  const chatPage = new ChatPage();
  const chatInput = new ChatInput();
  const header = new Header();
  const startupPage = new StartupPage();

  before(async () => {
    await browser.pause(3000);
    await header.waitForLoad();
  });

  describe('Workspace state', () => {
    it('should detect current workspace state', async () => {
      const startupVisible = await startupPage.isVisible();
      if (startupVisible) {
        console.log('[Test] In startup mode, need to open workspace first');
      } else {
        console.log('[Test] Workspace already open');
      }
    });
  });

  describe('Chat input', () => {
    let hasWorkspace = false;

    before(async () => {
      const startupVisible = await startupPage.isVisible();
      hasWorkspace = !startupVisible;
      if (!hasWorkspace) {
        console.log('[Test] Skipping chat tests: open workspace first');
      }
    });

    it('chat input should exist', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }
      await chatPage.waitForLoad();
      const isVisible = await chatPage.isChatInputVisible();
      expect(isVisible).toBe(true);
    });

    it('chat input component should be visible', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }
      await chatInput.waitForLoad();
      const isVisible = await chatInput.isVisible();
      expect(isVisible).toBe(true);
    });

    it('should type in input', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }
      const testMessage = 'Test message';
      await chatInput.typeMessage(testMessage);
      const value = await chatInput.getValue();
      expect(value).toContain(testMessage);
      await chatInput.clear();
    });

    it('send button should be enabled when input has text', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }
      await chatInput.clear();
      let isEnabled = await chatInput.isSendButtonEnabled();
      expect(isEnabled).toBe(false);
      await chatInput.typeMessage('Test');
      await browser.pause(200);
      isEnabled = await chatInput.isSendButtonEnabled();
      expect(isEnabled).toBe(true);
      await chatInput.clear();
    });

    it('should get input placeholder', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }
      const placeholder = await chatInput.getPlaceholder();
      expect(placeholder).toBeDefined();
      expect(placeholder.length).toBeGreaterThan(0);
      console.log('[Test] Input placeholder:', placeholder);
    });

    it('should not send on Enter while IME composition is active', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const testMessage = 'IME composition guard message';
      await chatInput.clear();
      await chatInput.typeMessage(testMessage);
      await chatInput.focus();

      await chatInput.triggerCompositionStart();
      await browser.keys(['Enter']);
      await browser.pause(300);

      const valueWhileComposing = await chatInput.getValue();
      expect(valueWhileComposing).toContain(testMessage);

      await chatInput.triggerCompositionEnd();
      await browser.pause(180);
      await browser.keys(['Enter']);
      await browser.pause(800);

      const valueAfterComposition = await chatInput.getValue();
      expect(valueAfterComposition).toBe('');
    });
  });

  describe('Send message (no wait for response)', () => {
    let hasWorkspace = false;

    before(async () => {
      const startupVisible = await startupPage.isVisible();
      hasWorkspace = !startupVisible;
    });

    it('should send message without waiting for AI response', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }
      const testMessage = 'E2E test message - please ignore';
      const countBefore = await chatPage.getMessageCount();
      console.log('[Test] Message count before send:', countBefore);
      await chatInput.typeMessage(testMessage);
      await chatInput.clickSend();
      await browser.pause(1000);
      const valueAfter = await chatInput.getValue();
      expect(valueAfter).toBe('');
      console.log('[Test] Message sent');
    });
  });

  afterEach(async function () {
    if (this.currentTest?.state === 'failed') {
      await saveFailureScreenshot(this.currentTest.title);
    }
  });

  after(async () => {
    await saveScreenshot('basic-chat-complete');
  });
});
