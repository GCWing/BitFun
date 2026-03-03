/**
 * Page object for chat input (bottom message input area).
 */
import { BasePage } from '../BasePage';
import { browser, $ } from '@wdio/globals';

export class ChatInput extends BasePage {
  private selectors = {
    // Use actual frontend selectors with fallbacks
    container: '[data-testid="chat-input-container"], .chat-input-container, .chat-input',
    textarea: '[data-testid="chat-input-textarea"], .chat-input textarea, textarea[class*="chat"]',
    sendBtn: '[data-testid="chat-input-send-btn"], .chat-input__send-btn, button[class*="send"]',
    attachmentBtn: '[data-testid="chat-input-attachment-btn"], .chat-input__attachment-btn',
    cancelBtn: '[data-testid="chat-input-cancel-btn"], .chat-input__cancel-btn, button[class*="cancel"]',
  };

  async isVisible(): Promise<boolean> {
    const containerSelectors = [
      '[data-testid="chat-input-container"]',
      '.chat-input-container',
      '.chat-input',
    ];

    for (const selector of containerSelectors) {
      try {
        const element = await $(selector);
        const exists = await element.isExisting();
        if (exists) {
          return true;
        }
      } catch (e) {
        // Continue
      }
    }
    return false;
  }

  async waitForLoad(): Promise<void> {
    const containerSelectors = [
      '[data-testid="chat-input-container"]',
      '.chat-input-container',
      '.chat-input',
    ];

    for (const selector of containerSelectors) {
      try {
        const element = await $(selector);
        const exists = await element.isExisting();
        if (exists) {
          return;
        }
      } catch (e) {
        // Continue
      }
    }
    await this.wait(1000);
  }

  private async findTextarea(): Promise<WebdriverIO.Element | null> {
    const selectors = [
      '.rich-text-input[contenteditable="true"]',
      '.bitfun-chat-input__input-area [contenteditable="true"]',
      '[contenteditable="true"]',
      '[data-testid="chat-input-textarea"]',
      '.chat-input textarea',
      'textarea[class*="chat"]',
    ];

    for (const selector of selectors) {
      try {
        const element = await $(selector);
        const exists = await element.isExisting();
        if (exists) {
          console.log(`[ChatInput] Found input element with selector: ${selector}`);
          return element;
        }
      } catch (e) {
        // Continue
      }
    }

    console.log('[ChatInput] Input element not found with any selector');
    return null;
  }

  async typeMessage(message: string): Promise<void> {
    const input = await this.findTextarea();
    if (input) {
      // For contentEditable elements, we need to use a different approach
      const isContentEditable = await input.getAttribute('contenteditable');

      if (isContentEditable === 'true') {
        // Click to focus first
        await input.click();
        await browser.pause(200);

        // Clear existing content first
        await browser.keys(['Control', 'a']);
        await browser.pause(100);
        await browser.keys(['Backspace']);
        await browser.pause(100);

        // Type the message, handling newlines
        if (message.includes('\n')) {
          // For multiline, split by newline and type with Shift+Enter
          const lines = message.split('\n');
          for (let i = 0; i < lines.length; i++) {
            for (const char of lines[i]) {
              await browser.keys([char]);
              await browser.pause(10);
            }
            // Add newline except after last line
            if (i < lines.length - 1) {
              await browser.keys(['Shift', 'Enter']);
              await browser.pause(50);
            }
          }
        } else {
          // Single line - type character by character
          for (const char of message) {
            await browser.keys([char]);
            await browser.pause(10);
          }
        }
        await browser.pause(200);
      } else {
        // Regular textarea
        await input.setValue(message);
        await browser.pause(200);
      }
    } else {
      throw new Error('Chat input element not found');
    }
  }

  async getValue(): Promise<string> {
    const input = await this.findTextarea();
    if (input) {
      const isContentEditable = await input.getAttribute('contenteditable');

      if (isContentEditable === 'true') {
        // For contentEditable, get textContent
        return await input.getText();
      } else {
        // Regular textarea
        return await input.getValue();
      }
    }
    return '';
  }

  async clear(): Promise<void> {
    const input = await this.findTextarea();
    if (input) {
      const isContentEditable = await input.getAttribute('contenteditable');

      if (isContentEditable === 'true') {
        // For contentEditable, select all and delete
        await input.click();
        await browser.pause(50);
        await browser.keys(['Control', 'a']);
        await browser.pause(50);
        await browser.keys(['Backspace']);
        await browser.pause(50);
      } else {
        // Regular textarea
        await input.clearValue();
      }
    }
  }

  async clickSend(): Promise<void> {
    const selectors = [
      '[data-testid="chat-input-send-btn"]',
      '.chat-input__send-btn',
      'button[class*="send"]',
      'button[aria-label*="send" i]',
      'button[aria-label*="发送" i]',
    ];

    for (const selector of selectors) {
      try {
        const element = await $(selector);
        const exists = await element.isExisting();
        if (exists) {
          const isEnabled = await this.isSendButtonEnabled();
          if (isEnabled) {
            await element.click();
            await browser.pause(500); // Wait for the action to complete
            return;
          } else {
            console.log('[ChatInput] Send button is disabled, cannot click');
            return;
          }
        }
      } catch (e) {
        // Continue
      }
    }
    // Fallback: press Ctrl+Enter (more reliable than just Enter for sending)
    console.log('[ChatInput] Send button not found, using Ctrl+Enter as fallback');
    await browser.keys(['Control', 'Enter']);
    await browser.pause(500);
  }

  async isSendButtonEnabled(): Promise<boolean> {
    const selectors = [
      '[data-testid="chat-input-send-btn"]',
      '.chat-input__send-btn',
      'button[class*="send"]',
      'button[aria-label*="send" i]',
      'button[aria-label*="发送" i]',
    ];

    for (const selector of selectors) {
      try {
        const element = await $(selector);
        const exists = await element.isExisting();
        if (exists) {
          const isEnabled = await element.isEnabled();
          const isDisabled = await element.getAttribute('disabled');
          const ariaDisabled = await element.getAttribute('aria-disabled');
          
          // Check multiple disabled states
          const actuallyEnabled = isEnabled && !isDisabled && ariaDisabled !== 'true';
          
          console.log(`[ChatInput] Send button state: enabled=${isEnabled}, disabled=${isDisabled}, aria-disabled=${ariaDisabled}, actuallyEnabled=${actuallyEnabled}`);
          return actuallyEnabled;
        }
      } catch (e) {
        // Continue
      }
    }
    return false;
  }

  async isSendButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.sendBtn);
  }

  async sendMessage(message: string): Promise<void> {
    await this.typeMessage(message);
    await this.clickSend();
  }

  async sendMessageWithEnter(message: string): Promise<void> {
    await this.typeMessage(message);
    await browser.keys(['Enter']);
  }

  async sendMessageWithCtrlEnter(message: string): Promise<void> {
    await this.typeMessage(message);
    await browser.keys(['Control', 'Enter']);
  }

  async clickAttachment(): Promise<void> {
    await this.safeClick(this.selectors.attachmentBtn);
  }

  async isAttachmentButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.attachmentBtn);
  }

  async clickCancel(): Promise<void> {
    await this.safeClick(this.selectors.cancelBtn);
  }

  async isCancelButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.cancelBtn);
  }

  async getPlaceholder(): Promise<string> {
    const input = await this.findTextarea();
    if (input) {
      // Try data-placeholder attribute first (for contentEditable)
      const dataPlaceholder = await input.getAttribute('data-placeholder');
      if (dataPlaceholder) {
        return dataPlaceholder;
      }

      // Fallback to placeholder attribute (for textarea)
      return (await input.getAttribute('placeholder')) || '';
    }
    return '';
  }

  async focus(): Promise<void> {
    const input = await this.findTextarea();
    if (input) {
      await input.click();
    }
  }

  async isFocused(): Promise<boolean> {
    const input = await this.findTextarea();
    if (input) {
      return await input.isFocused();
    }
    return false;
  }
}

export default ChatInput;
