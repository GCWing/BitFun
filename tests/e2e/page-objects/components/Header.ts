/**
 * Page object for header (title bar and window controls).
 */
import { BasePage } from '../BasePage';
import { $ } from '@wdio/globals';

export class Header extends BasePage {
  private selectors = {
    // Use actual frontend class names - NavBar uses bitfun-nav-bar class
    container: '.bitfun-nav-bar, [data-testid="header-container"], .bitfun-header, header',
    homeBtn: '[data-testid="header-home-btn"], .bitfun-nav-bar__logo-button, .bitfun-header__home',
    minimizeBtn: '[data-testid="header-minimize-btn"], .bitfun-title-bar__minimize',
    maximizeBtn: '[data-testid="header-maximize-btn"], .bitfun-title-bar__maximize',
    closeBtn: '[data-testid="header-close-btn"], .bitfun-title-bar__close',
    leftPanelToggle: '[data-testid="header-left-panel-toggle"], .bitfun-nav-bar__panel-toggle',
    rightPanelToggle: '[data-testid="header-right-panel-toggle"]',
    newSessionBtn: '[data-testid="header-new-session-btn"]',
    title: '[data-testid="header-title"], .bitfun-nav-bar__menu-item-main, .bitfun-header__title',
    configBtn: '[data-testid="header-config-btn"], .bitfun-header-right button',
  };

  async isVisible(): Promise<boolean> {
    const selectors = ['.bitfun-nav-bar', '[data-testid="header-container"]', '.bitfun-header', 'header'];
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

  async waitForLoad(): Promise<void> {
    // Wait for any header element - NavBar uses bitfun-nav-bar class
    const selectors = ['.bitfun-nav-bar', '[data-testid="header-container"]', '.bitfun-header', 'header'];
    for (const selector of selectors) {
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
    // Fallback wait
    await this.wait(2000);
  }

  async clickHome(): Promise<void> {
    await this.safeClick(this.selectors.homeBtn);
  }

  async isHomeButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.homeBtn);
  }

  async clickMinimize(): Promise<void> {
    await this.safeClick(this.selectors.minimizeBtn);
  }

  async isMinimizeButtonVisible(): Promise<boolean> {
    // Check for window controls in various possible locations
    const selectors = [
      '[data-testid="header-minimize-btn"]',
      '.bitfun-title-bar__minimize',
      '.window-controls button:first-child',
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

  async clickMaximize(): Promise<void> {
    await this.safeClick(this.selectors.maximizeBtn);
  }

  async isMaximizeButtonVisible(): Promise<boolean> {
    const selectors = [
      '[data-testid="header-maximize-btn"]',
      '.bitfun-title-bar__maximize',
      '.window-controls button:nth-child(2)',
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

  async clickClose(): Promise<void> {
    await this.safeClick(this.selectors.closeBtn);
  }

  async isCloseButtonVisible(): Promise<boolean> {
    const selectors = [
      '[data-testid="header-close-btn"]',
      '.bitfun-title-bar__close',
      '.window-controls button:last-child',
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

  async toggleLeftPanel(): Promise<void> {
    await this.safeClick(this.selectors.leftPanelToggle);
  }

  async toggleRightPanel(): Promise<void> {
    await this.safeClick(this.selectors.rightPanelToggle);
  }

  async clickNewSession(): Promise<void> {
    await this.safeClick(this.selectors.newSessionBtn);
  }

  async isNewSessionButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.newSessionBtn);
  }

  async getTitle(): Promise<string> {
    try {
      const element = await $(this.selectors.title);
      const exists = await element.isExisting();
      if (exists) {
        return await element.getText();
      }
    } catch (e) {
      // Return empty string
    }
    return '';
  }

  async areWindowControlsVisible(): Promise<boolean> {
    // In Tauri apps, window controls might be handled by the OS
    // Check if any window control elements exist
    const minimizeVisible = await this.isMinimizeButtonVisible();
    const maximizeVisible = await this.isMaximizeButtonVisible();
    const closeVisible = await this.isCloseButtonVisible();

    // If any control exists, consider controls visible
    return minimizeVisible || maximizeVisible || closeVisible;
  }
}

export default Header;
