/**
 * Page object for startup screen (no workspace open).
 */
import { BasePage } from './BasePage';
import { browser, $ } from '@wdio/globals';

export class StartupPage extends BasePage {
  private selectors = {
    // Use actual frontend class names
    container: '.welcome-scene--first-time, .welcome-scene, .bitfun-scene-viewport--welcome',
    openFolderBtn: '.welcome-scene__link-btn, .welcome-scene__primary-action',
    recentProjects: '.welcome-scene__recent-list',
    recentProjectItem: '.welcome-scene__recent-item',
    brandLogo: '.welcome-scene__logo-img',
    welcomeText: '.welcome-scene__greeting-label, .welcome-scene__workspace-title',
  };

  async waitForLoad(): Promise<void> {
    await this.waitForPageLoad();
    await this.wait(500);
  }

  async isVisible(): Promise<boolean> {
    // Check multiple selectors
    const selectors = [
      '.welcome-scene--first-time',
      '.welcome-scene',
      '.bitfun-scene-viewport--welcome',
    ];

    for (const selector of selectors) {
      try {
        const element = await $(selector);
        const exists = await element.isExisting();
        if (exists) {
          return true;
        }
      } catch (e) {
        // Continue to next selector
      }
    }
    // Ensure we return false, not undefined
    return false;
  }

  async clickOpenFolder(): Promise<void> {
    await this.safeClick(this.selectors.openFolderBtn);
  }

  async isOpenFolderButtonVisible(): Promise<boolean> {
    // Check for any action button on welcome scene
    const selectors = [
      '.welcome-scene__link-btn',
      '.welcome-scene__primary-action',
      '.welcome-scene__session-btn',
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

  async getRecentProjects(): Promise<string[]> {
    const items = await browser.$$(this.selectors.recentProjectItem);
    const projects: string[] = [];

    for (const item of items) {
      try {
        const text = await item.getText();
        projects.push(text);
      } catch (e) {
        // Skip item if text cannot be retrieved
      }
    }

    return projects;
  }

  async clickRecentProject(index: number): Promise<void> {
    const items = await browser.$$(this.selectors.recentProjectItem);

    if (index >= items.length) {
      throw new Error(`Recent project index ${index} out of range (total: ${items.length})`);
    }

    await items[index].click();
  }

  async isBrandLogoVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.brandLogo);
  }

  async getWelcomeText(): Promise<string> {
    const selectors = [
      '.welcome-scene__greeting-label',
      '.welcome-scene__workspace-title',
    ];

    for (const selector of selectors) {
      try {
        const element = await $(selector);
        const exists = await element.isExisting();
        if (exists) {
          return await element.getText();
        }
      } catch (e) {
        // Continue
      }
    }
    return '';
  }

  /**
   * Open a workspace by calling Tauri API directly
   * This bypasses the native file dialog for E2E testing
   */
  async openWorkspaceByPath(workspacePath: string): Promise<void> {
    try {
      console.log(`[StartupPage] Opening workspace: ${workspacePath}`);

      await browser.execute(async (path: string) => {
        const { workspaceManager } = await import('/src/infrastructure/services/business/workspaceManager.ts');
        await workspaceManager.openWorkspace(path);
      }, workspacePath);

      await browser.waitUntil(async () => {
        return browser.execute(async (targetPath: string) => {
          const { globalStateAPI } = await import('/src/shared/types/global-state.ts');
          const currentWorkspace = await globalStateAPI.getCurrentWorkspace();
          return currentWorkspace?.rootPath === targetPath;
        }, workspacePath);
      }, {
        timeout: 15000,
        interval: 500,
        timeoutMsg: `Workspace did not become active: ${workspacePath}`,
      });

      console.log('[StartupPage] Workspace opened successfully');
    } catch (error) {
      console.error('[StartupPage] Failed to open workspace:', error);
      throw error;
    }
  }

  /**
   * Check if a recent workspace exists and click it
   */
  async openRecentWorkspace(index: number = 0): Promise<boolean> {
    try {
      const recentProjects = await this.getRecentProjects();
      if (recentProjects.length > index) {
        await this.clickRecentProject(index);
        await this.wait(2000);
        return true;
      }
      return false;
    } catch (error) {
      console.error('[StartupPage] Failed to open recent workspace:', error);
      return false;
    }
  }
}

export default StartupPage;
