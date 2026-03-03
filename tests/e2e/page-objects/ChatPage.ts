/**
 * Page object for chat view (workspace mode).
 */
import { BasePage } from './BasePage';
import { browser, $, $$ } from '@wdio/globals';

export class ChatPage extends BasePage {
  private selectors = {
    // Use actual frontend selectors
    appLayout: '[data-testid="app-layout"], .bitfun-app-layout',
    mainContent: '[data-testid="app-main-content"], .bitfun-main-content',
    inputContainer: '[data-testid="chat-input-container"], .chat-input-container',
    textarea: '[data-testid="chat-input-textarea"], .chat-input textarea, textarea[class*="chat-input"]',
    sendBtn: '[data-testid="chat-input-send-btn"], .chat-input__send-btn, button[class*="send"]',
    messageList: '[data-testid="message-list"], .message-list, .chat-messages',
    userMessage: '[data-testid^="user-message-"], .user-message, [class*="user-message"]',
    modelResponse: '[data-testid^="model-response-"], .model-response, [class*="model-response"], [class*="assistant-message"]',
    modelSelector: '[data-testid="model-selector"], .model-selector, [class*="model-select"]',
    modelDropdown: '[data-testid="model-selector-dropdown"], .model-dropdown',
    toolCard: '[data-testid^="tool-card-"], .tool-card, [class*="tool-card"]',
    loadingIndicator: '[data-testid="loading-indicator"], .loading-indicator, [class*="loading"]',
    streamingIndicator: '[data-testid="streaming-indicator"], .streaming-indicator, [class*="streaming"]',
  };

  async waitForLoad(): Promise<void> {
    await this.waitForPageLoad();
    await this.wait(500);
  }

  async isChatInputVisible(): Promise<boolean> {
    const selectors = [
      '[data-testid="chat-input-container"]',
      '.chat-input-container',
      '.chat-input',
      'textarea[class*="chat"]',
    ];

    for (const selector of selectors) {
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

  async typeMessage(message: string): Promise<void> {
    const selectors = [
      '[data-testid="chat-input-textarea"]',
      '.chat-input textarea',
      'textarea[class*="chat-input"]',
    ];

    for (const selector of selectors) {
      try {
        const element = await $(selector);
        const exists = await element.isExisting();
        if (exists) {
          await element.setValue(message);
          return;
        }
      } catch (e) {
        // Continue
      }
    }
    throw new Error('Chat input textarea not found');
  }

  async clickSend(): Promise<void> {
    const selectors = [
      '[data-testid="chat-input-send-btn"]',
      '.chat-input__send-btn',
      'button[class*="send"]',
    ];

    for (const selector of selectors) {
      try {
        const element = await $(selector);
        const exists = await element.isExisting();
        if (exists) {
          await element.click();
          return;
        }
      } catch (e) {
        // Continue
      }
    }
    // Fallback: press Enter
    await browser.keys(['Enter']);
  }

  async sendMessage(message: string): Promise<void> {
    await this.typeMessage(message);
    await this.clickSend();
  }

  async getUserMessages(): Promise<string[]> {
    const messages = await $$(this.selectors.userMessage);
    const texts: string[] = [];

    for (const msg of messages) {
      try {
        const text = await msg.getText();
        texts.push(text);
      } catch (e) {
        // Skip
      }
    }

    return texts;
  }

  async getModelResponses(): Promise<string[]> {
    const responses = await $$(this.selectors.modelResponse);
    const texts: string[] = [];

    for (const resp of responses) {
      try {
        const text = await resp.getText();
        texts.push(text);
      } catch (e) {
        // Skip
      }
    }

    return texts;
  }

  async getLastModelResponse(): Promise<string> {
    const responses = await $$(this.selectors.modelResponse);

    if (responses.length === 0) {
      return '';
    }

    return responses[responses.length - 1].getText();
  }

  async getMessageCount(): Promise<{ user: number; model: number }> {
    const userMessages = await $$(this.selectors.userMessage);
    const modelResponses = await $$(this.selectors.modelResponse);

    return {
      user: userMessages.length,
      model: modelResponses.length,
    };
  }

  async isModelSelectorVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.modelSelector);
  }

  async clickModelSelector(): Promise<void> {
    await this.safeClick(this.selectors.modelSelector);
  }

  async waitForModelDropdown(): Promise<void> {
    await this.waitForElement(this.selectors.modelDropdown);
  }

  async getToolCardCount(): Promise<number> {
    const toolCards = await $$(this.selectors.toolCard);
    return toolCards.length;
  }

  async isLoading(): Promise<boolean> {
    return this.isElementVisible(this.selectors.loadingIndicator);
  }

  async isStreaming(): Promise<boolean> {
    return this.isElementVisible(this.selectors.streamingIndicator);
  }

  async waitForLoadingComplete(): Promise<void> {
    await browser.pause(1000);
  }

  async clearInput(): Promise<void> {
    const selectors = [
      '[data-testid="chat-input-textarea"]',
      '.chat-input textarea',
    ];

    for (const selector of selectors) {
      try {
        const element = await $(selector);
        const exists = await element.isExisting();
        if (exists) {
          await element.clearValue();
          return;
        }
      } catch (e) {
        // Continue
      }
    }
  }

  async getInputValue(): Promise<string> {
    const selectors = [
      '[data-testid="chat-input-textarea"]',
      '.chat-input textarea',
    ];

    for (const selector of selectors) {
      try {
        const element = await $(selector);
        const exists = await element.isExisting();
        if (exists) {
          return await element.getValue();
        }
      } catch (e) {
        // Continue
      }
    }
    return '';
  }

  async isSendButtonEnabled(): Promise<boolean> {
    const selectors = [
      '[data-testid="chat-input-send-btn"]',
      '.chat-input__send-btn',
    ];

    for (const selector of selectors) {
      try {
        const element = await $(selector);
        const exists = await element.isExisting();
        if (exists) {
          return await element.isEnabled();
        }
      } catch (e) {
        // Continue
      }
    }
    return false;
  }
}

export default ChatPage;
