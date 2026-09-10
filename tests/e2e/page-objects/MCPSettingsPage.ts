import { $, browser } from '@wdio/globals';
import { BasePage } from './BasePage';

export class MCPSettingsPage extends BasePage {
  get input() { return $('[data-testid="mcp-json-input"]'); }
  get saveButton() { return $('[data-testid="mcp-json-save"]'); }
  get warning() { return $('.notification-item--warning'); }

  async open(): Promise<void> {
    await this.clickByTestId('nav-footer-settings-item', 15000);
    await this.clickByTestId('nav-settings-open-item');
    await this.safeClick('[data-testid="settings-nav-page"][data-settings-page="tools.mcp"]', 15000);
    await this.waitForTestId('mcp-json-toggle');
  }

  async openEditor(): Promise<void> {
    await this.clickByTestId('mcp-json-toggle');
    await this.input.waitForDisplayed();
    // A disabled save button alone can mean "still saving". Wait until the
    // editable input is enabled before checking that the persisted draft is clean.
    await browser.waitUntil(() => browser.execute(() => {
      // Read-back temporarily replaces the editor with a loading state, so an
      // element handle captured before that reload no longer owns the input.
      const input = document.querySelector<HTMLTextAreaElement>('[data-testid="mcp-json-input"]');
      return Boolean(input && !input.disabled);
    }), { timeout: 45000, timeoutMsg: 'MCP editor did not finish reloading after save' });
  }

  async edit(json: string): Promise<void> {
    await this.input.setValue(json);
    await this.saveButton.waitForEnabled();
  }

  async save(): Promise<void> {
    await this.saveButton.click();
  }

  async closeEditor(): Promise<void> {
    await this.clickByTestId('mcp-json-toggle');
    await this.input.waitForDisplayed({ reverse: true });
  }

  async waitForConnected(serverId: string): Promise<void> {
    const row = await $(`[data-testid="mcp-server-item"][data-server-id="${serverId}"]`);
    // Stop is offered only for a connected/healthy runtime. Waiting on the
    // rendered control also proves that background recovery reaches the UI.
    await row.$('[data-testid="mcp-server-stop"]').waitForDisplayed({ timeout: 45000 });
  }

  async dismissWarning(): Promise<void> {
    if (await this.warning.isExisting()) {
      await this.warning.$('[data-openbitfun-part="itemClose"] button').click();
      await this.warning.waitForDisplayed({ reverse: true });
    }
  }

  async screenshot(file: string): Promise<void> {
    await browser.execute(async () => {
      // Capture the rendered state after entrance transitions, not a faded
      // intermediate animation frame. Leave perpetual status animations alone.
      await Promise.all(document.getAnimations()
        .filter(animation => animation.effect?.getComputedTiming().iterations !== Infinity)
        .map(animation => animation.finished.catch(() => undefined)));
    });
    await browser.saveScreenshot(file);
  }
}
