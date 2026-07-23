/**
 * Base class for all page objects. Common element queries, waits, and actions.
 */
import { browser, $ } from '@wdio/globals';
import { environmentSettings } from '../config/capabilities';

export class BasePage {
  async waitForPageLoad(timeout: number = environmentSettings.pageLoadTimeout): Promise<void> {
    await browser.waitUntil(
      async () => {
        const readyState = await browser.execute(() => document.readyState);
        return readyState === 'complete';
      },
      {
        timeout,
        timeoutMsg: `Page did not load within ${timeout}ms`,
      }
    );
  }

  async getByTestId(testId: string): Promise<WebdriverIO.Element> {
    return $(`[data-testid="${testId}"]`);
  }

  async waitForElement(
    selector: string,
    timeout: number = environmentSettings.defaultTimeout
  ): Promise<WebdriverIO.Element> {
    const element = await $(selector);
    await element.waitForDisplayed({
      timeout,
      timeoutMsg: `Element ${selector} not visible within ${timeout}ms`,
    });
    return element;
  }

  async waitForTestId(
    testId: string,
    timeout: number = environmentSettings.defaultTimeout
  ): Promise<WebdriverIO.Element> {
    return this.waitForElement(`[data-testid="${testId}"]`, timeout);
  }

  async safeClick(selector: string, timeout: number = environmentSettings.defaultTimeout): Promise<void> {
    const element = await this.waitForElement(selector, timeout);
    await element.waitForClickable({
      timeout,
      timeoutMsg: `Element ${selector} not clickable within ${timeout}ms`,
    });
    await element.click();
  }

  async clickByTestId(testId: string, timeout?: number): Promise<void> {
    await this.safeClick(`[data-testid="${testId}"]`, timeout);
  }

  async safeType(
    selector: string,
    text: string,
    timeout: number = environmentSettings.defaultTimeout
  ): Promise<void> {
    const element = await this.waitForElement(selector, timeout);
    await element.clearValue();
    await element.setValue(text);
  }

  async typeByTestId(testId: string, text: string, timeout?: number): Promise<void> {
    await this.safeType(`[data-testid="${testId}"]`, text, timeout);
  }

  async getText(selector: string, timeout?: number): Promise<string> {
    const element = await this.waitForElement(selector, timeout);
    return element.getText();
  }

  async getTextByTestId(testId: string, timeout?: number): Promise<string> {
    return this.getText(`[data-testid="${testId}"]`, timeout);
  }

  async isElementExist(selector: string): Promise<boolean> {
    const element = await $(selector);
    return element.isExisting();
  }

  async isElementVisible(selector: string): Promise<boolean> {
    const element = await $(selector);
    return element.isDisplayed();
  }

  async isTestIdVisible(testId: string): Promise<boolean> {
    return this.isElementVisible(`[data-testid="${testId}"]`);
  }

  async takeScreenshot(name: string): Promise<void> {
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    const fileName = `${name}-${timestamp}.png`;
    if (process.platform === 'linux') {
      console.warn(`Skipping screenshot on ${process.platform}: ${fileName}`);
      return;
    }
    await browser.saveScreenshot(`../reports/screenshots/${fileName}`);
    console.log(`Screenshot saved: ${fileName}`);
  }

  async wait(ms: number): Promise<void> {
    await browser.pause(ms);
  }

  async executeScript<T>(script: string | ((...args: unknown[]) => T), ...args: unknown[]): Promise<T> {
    return browser.execute(script as () => T, ...args);
  }

  async getTitle(): Promise<string> {
    return browser.getTitle();
  }

  async refresh(): Promise<void> {
    await browser.refresh();
  }

  async withRetry<T>(
    action: () => Promise<T>,
    maxRetries: number = environmentSettings.maxRetries,
    retryDelay: number = environmentSettings.retryDelay
  ): Promise<T> {
    let lastError: Error | null = null;
    
    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      try {
        return await action();
      } catch (error) {
        lastError = error as Error;
        console.warn(`Attempt ${attempt}/${maxRetries} failed: ${lastError.message}`);
        
        if (attempt < maxRetries) {
          await this.wait(retryDelay);
        }
      }
    }
    
    throw lastError;
  }
}

export default BasePage;
